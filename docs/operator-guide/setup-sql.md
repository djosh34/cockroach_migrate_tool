# Source & Destination Setup

Before starting the runner, you must prepare both databases: CockroachDB needs changefeeds configured, and PostgreSQL needs the correct permissions granted. The runner only receives webhook payloads — it does not create changefeeds, databases, schemas, or destination tables for you.

**Prerequisite:** TLS certificates must already exist before writing runner configs, since configuration references certificate paths. See [TLS Configuration](tls-configuration.md).

## Part 1: CockroachDB source

### Step 1: Enable rangefeeds

Run once per CockroachDB cluster:

```sql
SET CLUSTER SETTING kv.rangefeed.enabled = true;
```

This persists across restarts.

### Step 2: Capture a cursor per source database

For each source database, capture the current logical timestamp immediately before creating changefeeds:

```sql
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;
```

The result is a decimal like `1745877420457561000.0000000000`. Paste this value into every `CREATE CHANGEFEED` for that database. One cursor per database keeps all changefeeds aligned on the same start boundary.

### Step 3: Create one changefeed per mapping

For each mapping in your runner config:

```sql
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders
INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D'
WITH cursor = '1745877420457561000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';
```

| Option | Value | Purpose |
|--------|-------|---------|
| `cursor` | Decimal from step 2 | Consistent start point |
| `initial_scan` | `'yes'` | Snapshots existing data before streaming |
| `envelope` | `'enriched'` | Payload format the runner expects |
| `resolved` | Interval, e.g. `'5s'` | Watermark emission frequency |

#### Sink URL format

```
webhook-<base_url>/ingest/<mapping_id>?ca_cert=<percent-encoded-base64-cert>
```

- `<base_url>` — The externally reachable runner URL, e.g. `https://runner.example.internal:8443`
- `<mapping_id>` — Must exactly match the `id` field in the runner config (case-sensitive)
- `<percent-encoded-base64-cert>` — PEM CA certificate base64-encoded with no line breaks, then percent-encoded

Encode the CA certificate:

```bash
CA_CERT_B64=$(cat /config/certs/ca.crt | base64 -w0 | python3 -c 'import urllib.parse,sys; print(urllib.parse.quote(sys.stdin.read().strip()))')
```

#### HTTP sinks (development only)

When the runner uses `mode: http`, omit `ca_cert` and use `webhook-http://`:

```sql
CREATE CHANGEFEED FOR TABLE demo_a.public.customers
INTO 'webhook-http://runner.example.internal:8080/ingest/app-a'
WITH cursor = '1745877420457561000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';
```

### Worked example: two databases, two mappings

```sql
-- Connect to: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require

-- Step 1: Enable rangefeeds
SET CLUSTER SETTING kv.rangefeed.enabled = true;

-- Step 2: Database demo_a
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;
-- Result: 1745877420457561000.0000000000

-- Step 3: Mapping app-a
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders
INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D'
WITH cursor = '1745877420457561000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';

-- Step 2: Database demo_b
USE demo_b;
SELECT cluster_logical_timestamp() AS changefeed_cursor;
-- Result: 1745877420459999000.0000000000

-- Step 3: Mapping app-b
CREATE CHANGEFEED FOR TABLE demo_b.public.invoices, demo_b.public.invoice_lines
INTO 'webhook-https://runner.example.internal:8443/ingest/app-b?ca_cert=LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D'
WITH cursor = '1745877420459999000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';
```

### CockroachDB checklist

- [ ] `kv.rangefeed.enabled = true` is set on the source cluster
- [ ] One cursor captured per source database before creating changefeeds
- [ ] Every `CREATE CHANGEFEED` uses `cursor`, `initial_scan = 'yes'`, `envelope = 'enriched'`, and `resolved`
- [ ] Each sink URL ends with `/ingest/<mapping_id>` matching the runner config
- [ ] Table names are fully qualified (`database.schema.table`)
- [ ] The `ca_cert` query parameter is correctly percent-encoded
- [ ] The runner HTTPS endpoint is reachable from the source cluster

## Part 2: PostgreSQL destination

The runtime role needs specific permissions on each destination database. The runner creates the `_cockroach_migration_tool` helper schema and its tracking tables automatically — you only need to grant access.

### Step 1: Database-level grants

Run once per destination database and runtime role:

```sql
GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;
```

- `CONNECT` — Allows the role to log into the database
- `CREATE` — Allows the role to create `_cockroach_migration_tool`

### Step 2: Schema-level grants

Run once per mapped schema and runtime role:

```sql
GRANT USAGE ON SCHEMA public TO migration_user_a;
```

### Step 3: Table-level grants

Run once per mapped table and runtime role:

```sql
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.orders TO migration_user_a;
```

| Privilege | Why the runner needs it |
|-----------|------------------------|
| `SELECT` | Check existing rows during reconciliation |
| `INSERT` | Write new rows from changefeed events |
| `UPDATE` | Update rows when changefeeds carry modifications |
| `DELETE` | Delete rows when changefeeds carry deletion events |

### What the runner creates

After grants are in place, the runner automatically creates:

- Schema: `_cockroach_migration_tool`
- Tables: `stream_state`, `table_sync_state`, and per-mapping helper tables

These are owned by the runtime role — no additional grants needed.

### Worked example: two databases, two mappings

```sql
-- Destination database app_a
GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;
GRANT USAGE ON SCHEMA public TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.orders TO migration_user_a;

-- Destination database app_b
GRANT CONNECT, CREATE ON DATABASE app_b TO migration_user_b;
GRANT USAGE ON SCHEMA public TO migration_user_b;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.invoices TO migration_user_b;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.invoice_lines TO migration_user_b;
```

### PostgreSQL checklist

- [ ] Every destination database has `GRANT CONNECT, CREATE ON DATABASE <database> TO <role>`
- [ ] Every mapped schema has `GRANT USAGE ON SCHEMA <schema> TO <role>`
- [ ] Every mapped table has `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE <schema>.<table> TO <role>`
- [ ] The runtime role exists and can authenticate
- [ ] Destination databases, schemas, and tables all exist before the runner starts
- [ ] When adding tables to a mapping, grant privileges before restarting the runner

## SQL generator scripts

The repository includes scripts that generate the SQL above from a small YAML config. They produce auditable SQL files for manual execution.

### CockroachDB source SQL generator

`scripts/generate-cockroach-setup-sql.sh` reads a YAML config and renders `SET CLUSTER SETTING`, cursor capture, and `CREATE CHANGEFEED` statements. Output files: `cockroach-<database>-setup.sql` and `cockroach-all-setup.sql`.

Required config shape:

```yaml
cockroach:
  url: "postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require"
webhook:
  base_url: "https://runner.example.internal:8443"
  ca_cert_path: "ca.crt"
  resolved: "5s"
mappings:
  - id: "app-a"
    source:
      database: "demo_a"
      tables:
        - "public.customers"
        - "public.orders"
```

Usage:

```bash
./scripts/generate-cockroach-setup-sql.sh [--dry-run] [--output-dir ./output] ./cockroach-setup-config.yml
```

### PostgreSQL grants SQL generator

`scripts/generate-postgres-grants-sql.sh` reads a YAML config and renders per-database grant statements. Output files: `postgres-<database>-grants.sql` and `postgres-all-grants.sql`.

Required config shape:

```yaml
mappings:
  - id: "app-a"
    destination:
      database: "app_a"
      runtime_role: "migration_user_a"
      tables:
        - "public.customers"
        - "public.orders"
```

Usage:

```bash
./scripts/generate-postgres-grants-sql.sh [--dry-run] [--output-dir ./output] ./postgres-grants-config.yml
```

### Dependencies

Both scripts require `bash`, `envsubst`, and `sort`. The Cockroach script additionally requires `base64`. YAML parsing uses `yq` (preferred) or falls back to `python3`.

```bash
nix develop   # provides all dependencies
```

### Common flags

| Flag | Effect |
|------|--------|
| `--help` | Print usage |
| `--dry-run` | Print files that would be generated without writing |
| `--output-dir <path>` | Write output to a directory other than `./output` |

Both scripts fail fast with an `error:` message on missing files, absent keys, missing dependencies, or invalid table names.

## Order of operations

1. Enable rangefeeds on the CockroachDB cluster
2. Grant PostgreSQL destination permissions
3. Capture changefeed cursors
4. Create changefeeds pointing at `/ingest/{mapping_id}`
5. Start the runner

CockroachDB retries webhook deliveries, so changefeeds can be created before the runner is listening — data simply won't flow until the runner is reachable.
