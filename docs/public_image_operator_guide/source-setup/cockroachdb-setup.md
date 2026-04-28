# CockroachDB Source Setup

You must prepare the source CockroachDB cluster before starting the runner. The runner only receives webhook payloads — it does not create changefeeds for you.

## What you need

- CockroachDB SQL access with permission to change cluster settings and create changefeeds.
- The externally reachable HTTPS URL for the runner webhook (e.g. `runner.example.internal:8443`).
- The CA certificate (PEM) that CockroachDB will trust when posting to the runner webhook.
- The mapping IDs and source tables from your runner configuration.
- A chosen resolved timestamp interval (e.g. `5s`).

## Step 1: Enable rangefeeds

Run once per cluster, before any changefeed creation:

```sql
SET CLUSTER SETTING kv.rangefeed.enabled = true;
```

This is a cluster-wide setting. It persists across restarts and only needs to be run once.

## Step 2: Capture a cursor per source database

Run this once per source database, immediately before creating changefeeds for that database:

```sql
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;
```

The result is a decimal value like `1745877420457561000.0000000000`. Paste this exact value into every `CREATE CHANGEFEED` statement for that database.

> **Important:** Capture the cursor once per database and reuse it across all mappings that share that database. This keeps changefeeds aligned on the same start boundary.

## Step 3: Create one changefeed per mapping

For each mapping in the runner config, create one changefeed:

```sql
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders
INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D'
WITH cursor = '1745877420457561000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';
```

### Required changefeed options

| Option | Value | Why |
| ------ | ---- | --- |
| `cursor` | Decimal from `cluster_logical_timestamp()` | Starts the changefeed at a consistent point |
| `initial_scan` | `'yes'` | Snapshots existing data before live streaming |
| `envelope` | `'enriched'` | The webhook payload format the runner expects |
| `resolved` | Interval such as `'5s'` | How often resolved watermarks are emitted |

### Sink URL format

```
webhook-<base_url>/ingest/<mapping_id>?ca_cert=<percent-encoded-base64-cert>
```

| Component | What to substitute |
| --------- | ------------------ |
| `<base_url>` | Externally reachable runner URL, e.g. `https://runner.example.internal:8443` |
| `<mapping_id>` | The `id` field from the corresponding mapping in the runner config |
| `<percent-encoded-base64-cert>` | PEM-encoded CA cert, base64-encoded with no line breaks, then percent-encoded |

> A trailing slash on the base URL is normalized automatically, but prefer omitting it for clarity.

### Encoding the CA certificate

```bash
CA_CERT_B64=$(cat /config/certs/ca.crt | base64 -w0 | python3 -c 'import urllib.parse,sys; print(urllib.parse.quote(sys.stdin.read().strip()))')
echo "ca_cert=${CA_CERT_B64}"
```

### HTTP (non-TLS) sinks

When `webhook.mode` is `http` in the runner config, use an HTTP sink URL and omit `ca_cert`:

```sql
CREATE CHANGEFEED FOR TABLE demo_a.public.customers
INTO 'webhook-http://runner.example.internal:8080/ingest/app-a'
WITH
  cursor = '1745877420457561000.0000000000',
  initial_scan = 'yes',
  envelope = 'enriched',
  resolved = '5s';
```

> **Warning:** Use HTTP only in isolated, trusted networks. Production deployments should always use HTTPS.

## Worked example: two databases, two mappings

```sql
-- Connect to: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require

-- Step 1: Enable rangefeeds (once per cluster)
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

## Order of operations

1. Enable rangefeed on the CockroachDB cluster.
2. Create destination PostgreSQL grants (see [PostgreSQL destination grants](../destination-setup/postgresql-grants.md)).
3. Capture the changefeed cursor (`cluster_logical_timestamp()`).
4. Create the changefeeds pointing at the runner's `/ingest/{mapping_id}` endpoint.
5. Start the runner so it begins listening on `/ingest/{mapping_id}`.

CockroachDB retries webhook deliveries, so changefeeds can be created before the runner is running — but data will not flow until the runner is reachable.

## Checklist

- [ ] `kv.rangefeed.enabled = true` is set on the source cluster.
- [ ] One cursor captured per source database before creating changefeeds.
- [ ] Every `CREATE CHANGEFEED` uses `cursor`, `initial_scan = 'yes'`, `envelope = 'enriched'`, and `resolved`.
- [ ] Each sink URL ends with `/ingest/<mapping_id>` and the mapping ID matches the runner config.
- [ ] Table names in the changefeed are fully qualified as `database.schema.table`.
- [ ] The `ca_cert` query parameter contains properly percent-encoded base64 certificate data.
- [ ] The runner HTTPS endpoint is reachable from the source cluster.

## See also

- [Runner getting started](../runner/getting-started.md) — write runner config and start the runner
- [TLS reference](../tls-reference.md) — runner webhook TLS configuration
- [Troubleshooting](../troubleshooting.md) — common source and changefeed errors
