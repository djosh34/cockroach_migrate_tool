# README.md

# CockroachDB to PostgreSQL Migration Tool

Continuously migrates data from CockroachDB to PostgreSQL using changefeed webhooks, with built-in row-level data verification.

The **runner** receives changefeed batches and writes row mutations into PostgreSQL destination tables. The **verify-service** compares source and destination data to confirm migration correctness.

## Documentation

Everything you need — installation, configuration, setup guides, and troubleshooting — is in the **[Operator Guide](docs/operator-guide/index.md)**.

## Development

Use the repository flake for local development:

```bash
nix run .#check
nix run .#lint
nix run .#test
nix develop
```


# docs/operator-guide/index.md

# CockroachDB to PostgreSQL Migration Tool — Operator Guide

Two published container images — `runner-image` and `verify-image` — together form a complete CockroachDB-to-PostgreSQL migration pipeline.

## What problem does this solve?

You have data in CockroachDB that must move into PostgreSQL, and the migration cannot be a one-time snapshot. CockroachDB changefeeds emit every row mutation as a real-time stream. This tool receives those streams, writes them into PostgreSQL destination tables, and lets you verify that every row arrived correctly.

## How the pieces fit together

```
CockroachDB source
      │
      │  changefeeds push row batches
      ▼
┌──────────┐     ┌────────────────┐
│  runner   │────▶│  PostgreSQL    │
│  image    │     │  destination   │
└──────────┘     └────────────────┘
                      │
                      │  verify-service
                      │  reads both sides
                      ▼
               ┌────────────────┐
               │  verify-image   │
               │  checks every   │
               │  row matches    │
               └────────────────┘
```

| Image | Role | Registry |
|-------|------|----------|
| `runner-image` | Receives changefeed webhooks, writes rows into PostgreSQL | GHCR (primary), Quay (mirror) |
| `verify-image` | Compares source and destination data row-by-row | GHCR (primary), Quay (mirror) |

Both images are multi-platform (`linux/amd64`, `linux/arm64`) and tagged by full Git commit SHA.

## Operator workflow

1. **[Install the images](installation.md)** — Pull from GHCR, authenticate, understand tags.
2. **[Configure CockroachDB and PostgreSQL](setup-sql.md)** — Enable rangefeeds, create changefeeds, grant destination permissions.
3. **[Configure and run the runner](runner.md)** — Write YAML config, validate, start the webhook listener.
4. **[Configure and run the verify-service](verify-service.md)** — Write YAML config, validate, start the API.
5. **[Run verify jobs](verify-service.md#job-lifecycle)** — Start, poll, and stop verification jobs.
6. **[TLS setup](tls-configuration.md)** — Certificate configuration for all components.
7. **[Troubleshooting](troubleshooting.md)** — Diagnose common failures.

**Order matters:** CockroachDB changefeeds and PostgreSQL grants must be in place before the runner starts. CockroachDB retries webhook deliveries, so changefeeds can be created before the runner is reachable — but no data flows until the runner is listening.

## Pages

| Page | Covers |
|------|--------|
| [Installation](installation.md) | Pull commands, tags, GHCR/Quay, authentication, running containers, log format |
| [Source & Destination Setup](setup-sql.md) | CockroachDB changefeeds, PostgreSQL grants, SQL generator scripts |
| [Runner](runner.md) | CLI, configuration reference, HTTP endpoints, webhook payload format, Docker Compose |
| [Verify-Service](verify-service.md) | CLI, configuration reference, job lifecycle API, Docker Compose |
| [TLS Configuration](tls-configuration.md) | TLS settings for runner listener, runner destinations, verify listener, verify database connections |
| [Troubleshooting](troubleshooting.md) | Common failures and diagnostic steps |


# docs/operator-guide/installation.md

# Installation

Both images are published to the GitHub Container Registry (GHCR) on every push and mirrored to Quay. GHCR is the source of truth.

## Pull commands

```bash
docker pull ghcr.io/<github-owner>/runner-image:<git-sha>
docker pull ghcr.io/<github-owner>/verify-image:<git-sha>
```

Replace `<git-sha>` with a full 40-character commit SHA. Available tags are listed on the GHCR package pages:

- Runner: `https://github.com/<owner>/cockroach_migrate_tool/pkgs/container/runner-image`
- Verify: `https://github.com/<owner>/cockroach_migrate_tool/pkgs/container/verify-image`

## Quay mirror

Images are copied to Quay after each GHCR publish. Quay repository names are controlled by the CI variables `RUNNER_IMAGE_REPOSITORY` and `VERIFY_IMAGE_REPOSITORY`; they may differ from the GHCR names. GHCR is the source of truth — always determine availability from GHCR, not Quay.

```
quay.io/<quay-organization>/<runner-repository>:<git-sha>
quay.io/<quay-organization>/<verify-repository>:<git-sha>
```

## Authentication

```bash
echo "$GITHUB_TOKEN" | docker login ghcr.io -u "$GITHUB_USERNAME" --password-stdin
```

The token requires the `read:packages` scope.

## Running a container

Both images default to their respective subcommand. Pass arguments after the image name.

### Validate runner config (offline)

```bash
docker run --rm \
  -v ./config:/config:ro \
  ghcr.io/<owner>/runner-image:<git-sha> \
  validate-config --config /config/runner.yml
```

### Validate runner config (deep — tests destination connectivity)

```bash
docker run --rm \
  -v ./config:/config:ro \
  --network host \
  ghcr.io/<owner>/runner-image:<git-sha> \
  validate-config --config /config/runner.yml --deep
```

### Start the runner

```bash
docker run --rm \
  -p 8443:8443 \
  -v ./config:/config:ro \
  ghcr.io/<owner>/runner-image:<git-sha> \
  run --config /config/runner.yml
```

### Validate verify-service config

```bash
docker run --rm \
  -v ./config:/config:ro \
  ghcr.io/<owner>/verify-image:<git-sha> \
  verify-service validate-config --config /config/verify-service.yml
```

### Start the verify-service

```bash
docker run --rm \
  -p 8080:8080 \
  -v ./config:/config:ro \
  ghcr.io/<owner>/verify-image:<git-sha> \
  verify-service run --config /config/verify-service.yml
```

> The verify image entrypoint is `molt` with default command `verify-service`. Always include both when overriding `command` in Docker Compose.

## Log format

Both images support `--log-format text|json`. The flag position differs:

| Image | Flag position | Example |
|-------|--------------|---------|
| `runner-image` | Global flag, before the subcommand | `--log-format json validate-config --config ...` |
| `verify-image` | Flag on the subcommand | `verify-service validate-config --log-format json --config ...` |

## Next steps

- [Source & Destination Setup](setup-sql.md) — CockroachDB changefeeds and PostgreSQL grants
- [Runner configuration](runner.md) — Full runner setup and config reference
- [Verify-service configuration](verify-service.md) — Full verify-service setup and config reference


# docs/operator-guide/runner.md

# Runner

The runner image receives CockroachDB changefeed webhook batches and writes the incoming row mutations into PostgreSQL destination tables. It also runs a periodic reconciliation loop over each destination table.

## Quick start

```bash
export RUNNER_IMAGE="ghcr.io/<owner>/runner-image:<git-sha>"
docker pull "${RUNNER_IMAGE}"

docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  run --config /config/runner.yml
```

Always validate config before running. See [full walkthrough](#end-to-end-walkthrough) below.

## CLI

```
runner [--log-format text|json] validate-config --config <PATH> [--deep]
runner [--log-format text|json] run --config <PATH>
```

| Subcommand | Purpose | Flags |
|------------|---------|-------|
| `validate-config` | Check config structure and field values (offline) | `--config <PATH>` (required), `--deep` (optional) |
| `run` | Start the webhook listener and reconciliation loop | `--config <PATH>` (required) |

- `--log-format json` outputs structured JSON on stderr. Default is `text`.
- `--deep` added to `validate-config` additionally verifies each destination database is reachable and every mapped table exists. Requires network access.

## Configuration reference

The runner reads a single YAML file passed via `--config <PATH>`.

### Top-level structure

```yaml
webhook: ...
reconcile: ...
mappings:
  - id: ...
    source: ...
    destination: ...
```

All three keys are required.

### `webhook`

Controls the listener that receives CockroachDB changefeed batches.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `bind_addr` | string | yes | Host and port, e.g. `0.0.0.0:8443` |
| `mode` | string | no | `http` or `https`. Defaults to `https`. |
| `tls` | object | yes for `mode: https` | Server TLS configuration. Must not appear when `mode: http`. |

#### `webhook.tls`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cert_path` | path | yes | Server certificate PEM file |
| `key_path` | path | yes | Server private key PEM file |
| `client_ca_path` | path | no | CA certificate for mTLS client verification |

#### Examples

HTTPS (production):

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

HTTPS with mTLS:

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

HTTP (development only):

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
```

### `reconcile`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `interval_secs` | integer | yes | Seconds between reconciliation passes. Must be > 0. |

```yaml
reconcile:
  interval_secs: 30
```

### `mappings`

A list of one or more mapping objects. Each mapping ties one source CockroachDB database and set of tables to one destination PostgreSQL connection.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Stable identifier for the `/ingest/<id>` route. Must be unique across all mappings. |
| `source` | object | yes | Source CockroachDB database and tables |
| `destination` | object | yes | Destination PostgreSQL connection |

#### `source`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `database` | string | yes | Source CockroachDB database name |
| `tables` | list of strings | yes | Schema-qualified table names, e.g. `public.customers`. At least one. Unique within the mapping. |

```yaml
source:
  database: demo_a
  tables:
    - public.customers
    - public.orders
```

#### `destination`

Two mutually exclusive forms: **URL** or **decomposed fields**. Never mix them.

##### URL form

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a
```

For TLS, add query parameters:

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt
```

##### Decomposed form

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `host` | string | yes | PostgreSQL hostname or IP |
| `port` | integer | yes | PostgreSQL port |
| `database` | string | yes | Target database name |
| `user` | string | yes | Database user |
| `password` | string | yes | Database password |
| `tls` | object | no | Destination TLS configuration |

```yaml
destination:
  host: pg-a.example.internal
  port: 5432
  database: app_a
  user: migration_user_a
  password: runner-secret-a
```

##### `destination.tls` (decomposed form only)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mode` | string | yes | `require`, `verify-ca`, or `verify-full` |
| `ca_cert_path` | path | required for `verify-ca` and `verify-full` | CA certificate for server verification |
| `client_cert_path` | path | no | Client certificate for mTLS. Must appear with `client_key_path`. |
| `client_key_path` | path | no | Client private key. Must appear with `client_cert_path`. |

| `mode` | Server certificate verified | `ca_cert_path` required |
|--------|----------------------------|------------------------|
| `require` | No | No |
| `verify-ca` | Yes (against CA) | Yes |
| `verify-full` | Yes (CA + hostname) | Yes |

```yaml
destination:
  host: pg-a.example.internal
  port: 5432
  database: app_a
  user: migration_user_a
  password: runner-secret-a
  tls:
    mode: verify-full
    ca_cert_path: /config/certs/destination-ca.crt
    client_cert_path: /config/certs/destination-client.crt
    client_key_path: /config/certs/destination-client.key
```

### Full example

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
      tls:
        mode: verify-full
        ca_cert_path: /config/certs/destination-ca.crt
        client_cert_path: /config/certs/destination-client.crt
        client_key_path: /config/certs/destination-client.key
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
    destination:
      url: postgresql://migration_user_b:runner-secret-b@pg-b.example.internal:5432/app_b?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt
```

## HTTP endpoints

All endpoints are served on the address configured in `webhook.bind_addr`.

### Health check

```
GET /healthz
```

Returns `200 OK` when the runner is alive.

```bash
curl -k https://runner.example.internal:8443/healthz
```

### Metrics

```
GET /metrics
```

Returns Prometheus-formatted metrics as `text/plain`. Metric names are prefixed with `cockroach_migration_tool_runner_`.

### Ingest

```
POST /ingest/{mapping_id}
Content-Type: application/json
```

The endpoint CockroachDB changefeeds post to. The `mapping_id` must exactly match a mapping `id` in the runner config.

#### Webhook payload format

Row batch:

```json
{
  "length": 2,
  "payload": [
    {
      "after": {"id": 1, "email": "first@example.com"},
      "key": {"id": 1},
      "op": "c",
      "source": {
        "database_name": "demo_a",
        "schema_name": "public",
        "table_name": "customers"
      }
    },
    {
      "key": {"id": 2},
      "op": "d",
      "source": {
        "database_name": "demo_a",
        "schema_name": "public",
        "table_name": "customers"
      }
    }
  ]
}
```

Resolved watermark:

```json
{"resolved": "1776526353000000000.0000000000"}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `length` | integer | yes | Must equal the number of entries in `payload` |
| `payload` | array | yes | Row events |
| `payload[].source.database_name` | string | yes | Database label |
| `payload[].source.schema_name` | string | yes | Schema label |
| `payload[].source.table_name` | string | yes | Table label |
| `payload[].op` | string | yes | `c` (create), `u` (update), `r` (refresh), `d` (delete) |
| `payload[].key` | object | yes | JSON key-column map |
| `payload[].after` | object | required for `c`, `u`, `r` | JSON post-change column map. Omit for `d`. |
| `resolved` | string | yes (watermark) | Non-empty resolved timestamp |

All events in a single batch must reference the same source table. `key` and `after` are arbitrary JSON column-value maps.

#### Response codes

| Status | Meaning |
|--------|---------|
| `200 OK` | Batch accepted |
| `400 Bad Request` | Malformed batch (e.g. length mismatch) |
| `404 Not Found` | Unknown `mapping_id` |
| `500 Internal Server Error` | Processing failure |

#### Manual test

```bash
curl -k -X POST \
  -H 'content-type: application/json' \
  -d '{"length":1,"payload":[{"after":{"id":1,"name":"test"},"key":{"id":1},"op":"c","source":{"database_name":"demo_a","schema_name":"public","table_name":"customers"}}]}' \
  https://localhost:8443/ingest/app-a
```

## End-to-end walkthrough

### 1. Pull the image

```bash
export RUNNER_IMAGE="ghcr.io/<owner>/runner-image:<git-sha>"
docker pull "${RUNNER_IMAGE}"
```

### 2. Write config

Create `config/runner.yml`. Minimal HTTPS example:

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
```

Minimal HTTP example (development only):

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      url: postgresql://migration_user_a:runner-secret-a@pg-a:5432/app_a
```

### 3. Validate

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml
```

With destination connectivity check:

```bash
docker run --rm \
  --network host \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml --deep
```

### 4. Start

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  run --config /config/runner.yml
```

With structured logging:

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  --log-format json \
  run --config /config/runner.yml
```

### 5. Verify

```bash
curl -k https://localhost:8443/healthz
```

## Docker Compose

`runner.compose.yml`:

```yaml
services:
  runner:
    image: "${RUNNER_IMAGE}"
    network_mode: bridge
    ports:
      - "${RUNNER_HTTPS_PORT:-8443}:8443"
    configs:
      - source: runner-config
        target: /config/runner.yml
      - source: runner-server-cert
        target: /config/certs/server.crt
      - source: runner-server-key
        target: /config/certs/server.key
    command:
      - --log-format
      - json
      - run
      - --config
      - /config/runner.yml

configs:
  runner-config:
    file: ./config/runner.yml
  runner-server-cert:
    file: ./config/certs/server.crt
  runner-server-key:
    file: ./config/certs/server.key
```

```bash
docker compose -f runner.compose.yml run --rm runner validate-config --config /config/runner.yml
docker compose -f runner.compose.yml up runner
```


# docs/operator-guide/setup-sql.md

# Source & Destination Setup

Before starting the runner, you must prepare both databases: CockroachDB needs changefeeds configured, and PostgreSQL needs the correct permissions granted. The runner only receives webhook payloads — it does not create changefeeds, databases, schemas, or destination tables for you.

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


# docs/operator-guide/tls-configuration.md

# TLS Configuration

Every TLS setting across the runner and verify-service, in one place. Use this page when configuring HTTPS listeners, mTLS, or database connections with certificate verification.

## Certificate mounting convention

Mount PEM-encoded certificates and keys under `/config/certs/...` inside containers. Config file paths reference these mount points:

```
/config/certs/server.crt
/config/certs/server.key
/config/certs/client-ca.crt
/config/certs/destination-ca.crt
/config/certs/destination-client.crt
/config/certs/destination-client.key
/config/certs/source-ca.crt
/config/certs/source-client.crt
/config/certs/source-client.key
```

## Runner: webhook listener

| Field | Purpose |
|-------|---------|
| `webhook.mode` | `http` or `https` (default `https`) |
| `webhook.tls.cert_path` | Server certificate |
| `webhook.tls.key_path` | Server private key |
| `webhook.tls.client_ca_path` | CA for mTLS (optional) |

### HTTP (development only)

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
```

No TLS block allowed. Only suitable for trusted local networks.

### HTTPS

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

### HTTPS with mTLS

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

The server verifies that connecting clients present a certificate signed by `client-ca.crt`.

## Runner: destination connection

Two mutually exclusive forms.

### URL form

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt&sslcert=/config/certs/destination-client.crt&sslkey=/config/certs/destination-client.key
```

| `sslmode` | Server verification |
|-----------|---------------------|
| `disable` | No TLS |
| `require` | TLS, no verification |
| `verify-ca` | Verify against CA |
| `verify-full` | Verify CA + hostname |

### Decomposed form with explicit `tls` block

```yaml
destination:
  host: pg-a.example.internal
  port: 5432
  database: app_a
  user: migration_user_a
  password: runner-secret-a
  tls:
    mode: verify-full
    ca_cert_path: /config/certs/destination-ca.crt
    client_cert_path: /config/certs/destination-client.crt
    client_key_path: /config/certs/destination-client.key
```

| `mode` | Server verification | `ca_cert_path` required |
|--------|---------------------|------------------------|
| `require` | TLS, no verification | No |
| `verify-ca` | Verify against CA | Yes |
| `verify-full` | Verify CA + hostname | Yes |

`client_cert_path` and `client_key_path` must always appear together.

## Verify-service: listener

| Field | Purpose |
|-------|---------|
| `listener.tls.cert_path` | Server certificate |
| `listener.tls.key_path` | Server private key |
| `listener.tls.client_ca_path` | CA for mTLS (optional) |

### HTTP

```yaml
listener:
  bind_addr: 0.0.0.0:8080
```

Omit the `tls` block entirely.

### HTTPS

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

### HTTPS with mTLS

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

When `tls` is present, `cert_path` and `key_path` are both required. `client_ca_path` is always optional.

## Verify-service: database connections

Both `verify.source` and `verify.destination` use the same shape: a URL with `sslmode` query parameter, plus an optional nested `tls` block for certificate file paths.

### Source with `verify-full` and client certificates

```yaml
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
```

### Destination with `verify-ca` (CA only)

```yaml
verify:
  destination:
    url: postgresql://verify_target:secret@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
```

### Passwordless client-certificate auth

Omit the password from the URL and supply both `client_cert_path` and `client_key_path`:

```yaml
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
```

### Rules

| Rule | Applies to |
|------|-----------|
| `sslmode=verify-ca` or `sslmode=verify-full` requires `tls.ca_cert_path` | Source, destination |
| `client_cert_path` and `client_key_path` must appear together | Source, destination |
| URL scheme must be `postgresql://` or `postgres://` | Source, destination |

## Quick reference: TLS field mapping

| Component | Config path | Fields |
|-----------|-------------|--------|
| Runner webhook listener | `webhook.mode`, `webhook.tls.*` | `cert_path`, `key_path`, `client_ca_path` (optional) |
| Runner destination (URL) | `mappings[].destination.url` | `sslmode`, `sslrootcert`, `sslcert`, `sslkey` in query params |
| Runner destination (decomposed) | `mappings[].destination.tls.*` | `mode`, `ca_cert_path`, `client_cert_path`, `client_key_path` |
| Verify listener | `listener.tls.*` | `cert_path`, `key_path`, `client_ca_path` (optional) |
| Verify source/destination | `verify.source.tls.*`, `verify.destination.tls.*` | `ca_cert_path`, `client_cert_path`, `client_key_path` |


# docs/operator-guide/troubleshooting.md

# Troubleshooting

Common failures and diagnostic steps.

## Runner

### `validate-config` exits nonzero

**Checks:**
- All three top-level keys (`webhook`, `reconcile`, `mappings`) are present.
- `webhook.bind_addr` is a valid `host:port` string.
- When `webhook.mode` is `https`, `webhook.tls` with `cert_path` and `key_path` is present.
- When `webhook.mode` is `http`, there is no `webhook.tls` block.
- `reconcile.interval_secs` is a positive integer.
- `mappings` contains at least one entry with a unique `id`.
- Each `mappings[].source.tables` entry is schema-qualified (`public.customers`, not `customers`).
- `mappings[].destination` uses either a `url` string or decomposed fields (`host`, `port`, `database`, `user`, `password`) — never both.
- When using decomposed `tls` with `mode: verify-ca` or `mode: verify-full`, `ca_cert_path` is set.
- `client_cert_path` and `client_key_path` always appear together.

### `validate-config --deep` cannot reach destination

**Checks:**
- The Docker container can reach the destination host and port. Use `--network host` if the database is on the host network.
- The destination PostgreSQL accepts connections from the runner container's IP.
- `sslmode` is correct. If the database requires TLS, use `verify-ca` or `verify-full` and mount the CA certificate.
- If using client certificates, both `client_cert_path` and `client_key_path` are mounted correctly.

### Changefeeds get connection refused

**Checks:**
- The runner is listening on an address reachable from the CockroachDB cluster. `0.0.0.0` is fine; `127.0.0.1` is only local.
- The CockroachDB cluster can resolve the runner hostname and reach the port.
- The `ca_cert` in the changefeed sink URL matches the CA that signed the runner's server certificate, properly percent-encoded.
- The sink URL uses `webhook-https://` (not `webhook-http://`) when the runner uses HTTPS mode.
- The runner container port is mapped correctly: `-p 8443:8443`.

### `POST /ingest/{mapping_id}` returns 404

**Checks:**
- The `mapping_id` in the changefeed sink URL exactly matches an `id` in the runner config (case-sensitive).
- The runner was restarted after the mapping was added.

### `POST /ingest/{mapping_id}` returns 400

**Checks:**
- The `length` field equals the number of entries in `payload`.
- All required fields are present in each payload entry: `key`, `op`, `source` (with `database_name`, `schema_name`, `table_name`), and `after` for `c`, `u`, `r` operations.

## Verify-Service

### `verify-service validate-config` exits nonzero

**Checks:**
- Both `listener` and `verify` keys are present.
- `listener.bind_addr` is a valid `host:port` string.
- If `listener.tls` is present, both `cert_path` and `key_path` are set.
- `verify.source.url` and `verify.destination.url` use `postgres://` or `postgresql://`.
- When `sslmode=verify-ca` or `sslmode=verify-full`, the corresponding `tls.ca_cert_path` is set.
- `client_cert_path` and `client_key_path` always appear as a pair.

### `POST /jobs` returns 409

A verify job is already running. Only one job at a time.

**Fix:** Poll `GET /jobs/{job_id}` until it finishes, or stop it with `POST /jobs/{job_id}/stop`.

### `GET /jobs/{job_id}` returns 404

The verify-service process restarted since the job was created. Job state is in-memory.

**Fix:** Start a new job with `POST /jobs`.

### Job fails with `source_access` error

**Checks:**
- The verify-service container can reach the source database host and port.
- The URL in `verify.source.url` is correct, including `sslmode`.
- All required TLS certificate files are mounted and paths in `verify.source.tls` match.
- The source PostgreSQL user has read permission on the tables being verified.

### Job fails with `destination_access` error

Same checks as `source_access`, applied to `verify.destination`.

### Job reports mismatches

1. Check `result.mismatch_summary.affected_tables` for affected tables.
2. Check `result.findings` for per-row detail: `mismatching_columns`, `source_values`, `destination_values`.
3. Decide whether to re-run verification after fixing data, or accept the mismatches.

## General

### Container cannot access mounted certificates ("file not found")

**Fix:**
- Verify the volume mount: `-v "$(pwd)/config:/config:ro"` maps local `./config` to `/config` inside the container.
- Verify file permissions — the container process needs read access to all certs and keys.
- Config paths must reference the container mount target (`/config/certs/server.crt`), not the host path.

### Port already in use

**Fix:**
- Change `webhook.bind_addr` or `listener.bind_addr` to a different port.
- Or map a different host port: `-p 9443:8443` instead of `-p 8443:8443`.

### Stale image

**Fix:**
- Force a fresh pull: `docker pull ghcr.io/<owner>/runner-image:<git-sha>`.
- Verify the image digest matches the published build.


# docs/operator-guide/verify-service.md

# Verify-Service

The verify-service image exposes an HTTP API for starting, polling, and stopping verification jobs that compare CockroachDB source data against PostgreSQL destination data row-by-row.

## Key constraints

- **Only one job runs at a time.** Starting a second job returns `409 Conflict`.
- **Only the most recent completed job is retained.** Starting a new job evicts the previous result.
- **Job state is in-memory.** All job history is lost on process restart. Previous job IDs return `404 Not Found`.

## Quick start

```bash
export VERIFY_IMAGE="ghcr.io/<owner>/verify-image:<git-sha>"
docker pull "${VERIFY_IMAGE}"

docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service run --config /config/verify-service.yml
```

> The verify image entrypoint is `molt` with default command `verify-service`. Always include `verify-service` when overriding `command` in Docker or Compose.

## CLI

```
verify-service validate-config --config <path> [--log-format text|json]
verify-service run --config <path> [--log-format text|json]
```

| Subcommand | Purpose | Flags |
|------------|---------|-------|
| `validate-config` | Check config structure and consistency | `--config <path>` (required) |
| `run` | Start the HTTP listener and accept verify jobs | `--config <path>` (required) |

`--log-format` is a flag on each subcommand, not a global flag. Defaults to `text`.

## Configuration reference

The verify-service reads a single YAML file passed via `--config <path>`.

### Top-level structure

```yaml
listener: ...
verify: ...
```

Both keys are required.

### `listener`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `bind_addr` | string | yes | Host and port, e.g. `0.0.0.0:8080` |
| `tls` | object | no | TLS configuration. Omit for plain HTTP. |

#### `listener.tls`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cert_path` | path | yes | Server certificate PEM file |
| `key_path` | path | yes | Server private key PEM file |
| `client_ca_path` | path | no | CA certificate for mTLS client verification |

When `tls` is present, `cert_path` and `key_path` are both required. When absent, the listener serves plain HTTP. `client_ca_path` is optional; when set, callers must present a client certificate signed by that CA.

#### Examples

HTTP:

```yaml
listener:
  bind_addr: 0.0.0.0:8080
```

HTTPS:

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

HTTPS with mTLS:

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

### `verify`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `source` | object | yes | Source (CockroachDB or PostgreSQL) database connection |
| `destination` | object | yes | Destination PostgreSQL database connection |
| `raw_table_output` | boolean | no | Enable `POST /tables/raw` endpoint. Defaults to `false`. |

#### `verify.source` and `verify.destination`

Both use the same shape:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | yes | Connection URL. Must use `postgresql://` or `postgres://` scheme. Include `sslmode` as a query parameter. |
| `tls` | object | no | File paths for TLS certificates and keys |

```yaml
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    url: postgresql://verify_target:secret@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
```

##### `tls` under source or destination

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `ca_cert_path` | path | required when `sslmode` is `verify-ca` or `verify-full` | CA certificate for server verification |
| `client_cert_path` | path | no | Client certificate for mTLS |
| `client_key_path` | path | no | Client private key. Must appear with `client_cert_path`. |

`sslmode` values:

| `sslmode` | Server verification | Requires `ca_cert_path` |
|-----------|---------------------|------------------------|
| `disable` | No TLS | No |
| `require` | TLS, no verification | No |
| `verify-ca` | TLS, verify against CA | Yes |
| `verify-full` | TLS, verify CA + hostname | Yes |

When `sslmode=verify-ca` or `sslmode=verify-full`, `ca_cert_path` is required. `client_cert_path` and `client_key_path` must always appear as a pair. For passwordless client-certificate auth, omit the password from the URL.

### Full example

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
verify:
  raw_table_output: true
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    url: postgresql://verify_target@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
      client_cert_path: /config/certs/destination-client.crt
      client_key_path: /config/certs/destination-client.key
```

## Job lifecycle

### Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/jobs` | Start a verify job |
| `GET` | `/jobs/{job_id}` | Poll job status |
| `POST` | `/jobs/{job_id}/stop` | Request cancellation |
| `POST` | `/tables/raw` | Read raw table rows |
| `GET` | `/metrics` | Prometheus metrics |

### Start a job

```
POST /jobs
Content-Type: application/json
```

Request body (all fields optional POSIX regular expressions):

```json
{
  "include_schema": "^public$",
  "include_table": "^(accounts|orders)$"
}
```

| Field | Description |
|-------|-------------|
| `include_schema` | Include schemas matching this regex |
| `include_table` | Include tables matching this regex |
| `exclude_schema` | Exclude schemas matching this regex |
| `exclude_table` | Exclude tables matching this regex |

To verify everything, send `{}`.

**Accepted — `202`:**

```json
{"job_id": "job-000001", "status": "running"}
```

**Already running — `409 Conflict`:**

```json
{"error": {"category": "job_state", "code": "job_already_running", "message": "a verify job is already running"}}
```

**Validation error — `400`:**

```json
{"error": {"category": "request_validation", "code": "unknown_field", "message": "request body contains an unsupported field", "details": [{"field": "extra", "reason": "unknown field"}]}}
```

### Poll job status

```
GET /jobs/{job_id}
```

Poll every 2 seconds until status is no longer `running` or `stopping`.

**Running — `200 OK`:**

```json
{"job_id": "job-000001", "status": "running"}
```

**Succeeded — `200 OK`:**

```json
{
  "job_id": "job-000001",
  "status": "succeeded",
  "result": {
    "summary": {
      "tables_verified": 1,
      "tables_with_data": 1,
      "has_mismatches": false
    },
    "table_summaries": [
      {
        "schema": "public",
        "table": "accounts",
        "num_verified": 7,
        "num_success": 7,
        "num_missing": 0,
        "num_mismatch": 0,
        "num_column_mismatch": 0,
        "num_extraneous": 0,
        "num_live_retry": 0
      }
    ],
    "findings": [],
    "mismatch_summary": {
      "has_mismatches": false,
      "affected_tables": [],
      "counts_by_kind": {}
    }
  }
}
```

**Failed with mismatches — `200 OK`:**

```json
{
  "job_id": "job-000001",
  "status": "failed",
  "failure": {
    "category": "mismatch",
    "code": "mismatch_detected",
    "message": "verify detected mismatches in 1 table",
    "details": [{"reason": "mismatch detected for public.accounts"}]
  },
  "result": {
    "summary": {
      "tables_verified": 1,
      "tables_with_data": 1,
      "has_mismatches": true
    },
    "table_summaries": [
      {
        "schema": "public",
        "table": "accounts",
        "num_verified": 7,
        "num_success": 6,
        "num_missing": 0,
        "num_mismatch": 0,
        "num_column_mismatch": 1,
        "num_extraneous": 0,
        "num_live_retry": 0
      }
    ],
    "findings": [
      {
        "kind": "mismatching_column",
        "schema": "public",
        "table": "accounts",
        "primary_key": {"id": "101"},
        "mismatching_columns": ["balance"],
        "source_values": {"balance": "17"},
        "destination_values": {"balance": "23"},
        "info": ["balance mismatch"]
      }
    ],
    "mismatch_summary": {
      "has_mismatches": true,
      "affected_tables": [{"schema": "public", "table": "accounts"}],
      "counts_by_kind": {"mismatching_column": 1}
    }
  }
}
```

**Failed with connection error — `200 OK`:**

```json
{
  "job_id": "job-000001",
  "status": "failed",
  "failure": {
    "category": "source_access",
    "code": "connection_failed",
    "message": "source connection failed: dial tcp 127.0.0.1:5432: connect: connection refused",
    "details": [{"reason": "dial tcp 127.0.0.1:5432: connect: connection refused"}]
  }
}
```

**Not found — `404`:**

```json
{"error": {"category": "job_state", "code": "job_not_found", "message": "job not found"}}
```

### Stop a job

```
POST /jobs/{job_id}/stop
Content-Type: application/json

{}
```

Immediate response — `200 OK`:

```json
{"job_id": "job-000001", "status": "stopping"}
```

The job transitions to `stopped` asynchronously. Poll until status is `stopped`.

### Metrics

```
GET /metrics
```

Returns Prometheus-formatted metrics as `text/plain`. Metric names are prefixed with `cockroach_migration_tool_verify_`.

### Raw table read

```
POST /tables/raw
Content-Type: application/json

{"database": "source", "schema": "public", "table": "accounts"}
```

Only available when `verify.raw_table_output` is `true`. Returns `403` if disabled.

### Job states

| Status | Meaning | Terminal |
|--------|---------|----------|
| `running` | Job is actively verifying | No |
| `stopping` | Stop requested, winding down | No |
| `succeeded` | Verification completed, no mismatches | Yes |
| `failed` | Completed with mismatches or error | Yes |
| `stopped` | Cancelled by operator | Yes |

### Error categories

| Category | When it occurs |
|----------|---------------|
| `request_validation` | Invalid filter, unknown field, body too large |
| `job_state` | Job already running, job not found |
| `source_access` | Cannot connect to source database |
| `mismatch` | Mismatches detected during verification |
| `verify_execution` | Internal verify execution failure |

### Interpreting results

1. Check `result.summary.has_mismatches`.
2. If `true`, inspect `result.mismatch_summary.affected_tables`.
3. For per-row detail, check `result.findings` — each finding includes `mismatching_columns`, `source_values`, and `destination_values`.

## End-to-end walkthrough

### 1. Pull the image

```bash
export VERIFY_IMAGE="ghcr.io/<owner>/verify-image:<git-sha>"
docker pull "${VERIFY_IMAGE}"
```

### 2. Write config

Create `config/verify-service.yml`. Minimal HTTP example:

```yaml
listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
  destination:
    url: postgresql://verify_target@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
```

HTTPS with mTLS:

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    url: postgresql://verify_target:secret@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
```

### 3. Validate

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service validate-config --config /config/verify-service.yml
```

### 4. Start

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service run --config /config/verify-service.yml
```

With structured logging:

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service run --log-format json --config /config/verify-service.yml
```

### 5. Run a verify job

```bash
export VERIFY_API="http://localhost:8080"

# Start
JOB_ID=$(curl --silent --show-error \
  -H 'content-type: application/json' \
  -d '{"include_schema":"^public$","include_table":"^(accounts|orders)$"}' \
  "${VERIFY_API}/jobs" | jq -r '.job_id')

# Poll
curl --silent --show-error \
  "${VERIFY_API}/jobs/${JOB_ID}"

# Stop if needed
curl --silent --show-error \
  -H 'content-type: application/json' \
  -d '{}' \
  -X POST "${VERIFY_API}/jobs/${JOB_ID}/stop"
```

## Docker Compose

`verify.compose.yml`:

```yaml
services:
  verify:
    image: "${VERIFY_IMAGE}"
    network_mode: bridge
    ports:
      - "${VERIFY_HTTPS_PORT:-9443}:8443"
    configs:
      - source: verify-service-config
        target: /config/verify-service.yml
      - source: verify-source-ca
        target: /config/certs/source-ca.crt
      - source: verify-source-client-cert
        target: /config/certs/source-client.crt
      - source: verify-source-client-key
        target: /config/certs/source-client.key
      - source: verify-destination-ca
        target: /config/certs/destination-ca.crt
      - source: verify-client-ca
        target: /config/certs/client-ca.crt
      - source: verify-server-cert
        target: /config/certs/server.crt
      - source: verify-server-key
        target: /config/certs/server.key
    command:
      - verify-service
      - run
      - --log-format
      - json
      - --config
      - /config/verify-service.yml

configs:
  verify-service-config:
    file: ./config/verify-service.yml
  verify-source-ca:
    file: ./config/certs/source-ca.crt
  verify-source-client-cert:
    file: ./config/certs/source-client.crt
  verify-source-client-key:
    file: ./config/certs/source-client.key
  verify-destination-ca:
    file: ./config/certs/destination-ca.crt
  verify-client-ca:
    file: ./config/certs/client-ca.crt
  verify-server-cert:
    file: ./config/certs/server.crt
  verify-server-key:
    file: ./config/certs/server.key
```

```bash
docker compose -f verify.compose.yml up verify
```
