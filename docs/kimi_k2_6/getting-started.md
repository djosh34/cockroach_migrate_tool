# Getting Started

This guide will get you running a CockroachDB-to-PostgreSQL migration in minutes.

## What this project does

The migration tool streams changes from a **CockroachDB** source cluster into a **PostgreSQL** destination using CockroachDB's native [changefeed](https://www.cockroachlabs.com/docs/stable/change-data-capture-overview) feature. A long-running `runner` process receives row-level changes via HTTPS webhooks, stores them in temporary shadow tables, and periodically reconciles those changes into the real destination tables.

## Quick Start with Docker

The fastest way to try the tool is with the provided Docker Compose files.

### 1. Prepare configuration files

Create a working directory and add the three YAML files below.

#### `runner.yml` — runtime configuration

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: demo
    source:
      database: crdb_demo
      tables:
        - public.customers
        - public.orders
    destination:
      host: pg.example.internal
      port: 5432
      database: pg_demo
      user: migration_user
      password: ${PG_PASSWORD}
```

#### `cockroach-setup.yml` — changefeed bootstrap configuration

```yaml
cockroach:
  url: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require
webhook:
  base_url: https://runner.example.internal:8443
  ca_cert_path: /config/ca.crt
  resolved: 5s
mappings:
  - id: demo
    source:
      database: crdb_demo
      tables:
        - public.customers
        - public.orders
```

#### `postgres-grants.yml` — destination permission configuration

```yaml
mappings:
  - id: demo
    destination:
      database: pg_demo
      runtime_role: migration_user
      tables:
        - public.customers
        - public.orders
```

### 2. Generate the CockroachDB changefeed SQL

Use the `setup-sql` image to emit the SQL you will run against your CockroachDB cluster.

```bash
docker run --rm \
  -v $(pwd)/cockroach-setup.yml:/config/cockroach-setup.yml:ro \
  -v $(pwd)/ca.crt:/config/ca.crt:ro \
  setup-sql:latest \
  emit-cockroach-sql \
  --config /config/cockroach-setup.yml
```

The output will look similar to this:

```sql
-- Source bootstrap SQL
-- Cockroach URL: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require
-- Apply each statement with a Cockroach SQL client against the source cluster.
-- Capture the cursor once, then replace __CHANGEFEED_CURSOR__ in the CREATE CHANGEFEED statements below.

SET CLUSTER SETTING kv.rangefeed.enabled = true;
SELECT cluster_logical_timestamp() AS changefeed_cursor;

-- Source database: crdb_demo

-- Mapping: demo
-- Selected tables: public.customers, public.orders
-- Replace __CHANGEFEED_CURSOR__ below with the decimal cursor returned above before running the CREATE CHANGEFEED statement.
CREATE CHANGEFEED FOR TABLE crdb_demo.public.customers, crdb_demo.public.orders INTO 'webhook-https://runner.example.internal:8443/ingest/demo?ca_cert=...' WITH cursor = '__CHANGEFEED_CURSOR__', initial_scan = 'yes', envelope = 'enriched', enriched_properties = 'source', resolved = '5s';
```

Run the printed SQL against your CockroachDB cluster (replace `__CHANGEFEED_CURSOR__` with the value returned by `SELECT cluster_logical_timestamp()`).

### 3. Generate PostgreSQL grants (optional)

If you prefer to script destination permissions:

```bash
docker run --rm \
  -v $(pwd)/postgres-grants.yml:/config/postgres-grants.yml:ro \
  setup-sql:latest \
  emit-postgres-grants \
  --config /config/postgres-grants.yml
```

Run the emitted `GRANT` statements against your PostgreSQL instance so the runner role can read and write the destination tables.

### 4. Start the runner

```bash
docker run -d \
  --name migration-runner \
  -p 8443:8443 \
  -v $(pwd)/runner.yml:/config/runner.yml:ro \
  -v $(pwd)/server.crt:/config/certs/server.crt:ro \
  -v $(pwd)/server.key:/config/certs/server.key:ro \
  runner:latest \
  run \
  --log-format json \
  --config /config/runner.yml
```

The runner will:
1. Validate the configuration.
2. Connect to PostgreSQL and create a helper schema (`_cockroach_migration_tool`) with shadow tables.
3. Open an HTTPS webhook listener on port `8443`.
4. Start a background reconcile loop that flushes shadow data into the real tables every `interval_secs`.

### 5. Verify health and metrics

```bash
# Health check
curl -k https://localhost:8443/healthz
# → ok

# Prometheus-compatible metrics
curl -k https://localhost:8443/metrics
```

## Configuration reference

### Runner configuration (`runner.yml`)

| Section | Field | Required | Description |
|---------|-------|----------|-------------|
| `webhook` | `bind_addr` | yes | Socket address to listen on (e.g. `0.0.0.0:8443`). |
| `webhook` | `mode` | no | `http` or `https` (default `https`). |
| `webhook` | `tls` | conditional | Required when `mode: https`. Contains `cert_path`, `key_path`, and optional `client_ca_path` for mTLS. |
| `reconcile` | `interval_secs` | yes | How often (in seconds) to flush shadow tables into real tables. Must be > 0. |
| `mappings` | `id` | yes | Unique identifier for this mapping. Used in the webhook path (`/ingest/{id}`). |
| `mappings` | `source.database` | yes | CockroachDB source database name. |
| `mappings` | `source.tables` | yes | List of schema-qualified tables (e.g. `public.customers`). |
| `mappings` | `destination` | yes | Either a `url` string or decomposed fields (`host`, `port`, `database`, `user`, `password`, optional `tls`). |

**Destination TLS (`mappings.destination.tls`)**

```yaml
destination:
  host: pg.example.internal
  port: 5432
  database: app
  user: app_user
  password: secret
  tls:
    mode: verify-full          # require | verify-ca | verify-full
    ca_cert_path: /config/ca.crt
    client_cert_path: /config/client.crt
    client_key_path: /config/client.key
```

### Setup-SQL configuration (`cockroach-setup.yml`)

| Section | Field | Required | Description |
|---------|-------|----------|-------------|
| `cockroach` | `url` | yes | Connection string to the CockroachDB cluster. |
| `webhook` | `base_url` | yes | HTTPS URL prefix of the runner webhook listener. |
| `webhook` | `ca_cert_path` | yes | Path to the CA certificate that signed the runner webhook certificate. |
| `webhook` | `resolved` | yes | Changefeed `resolved` interval (e.g. `5s`). |
| `mappings` | `id` | yes | Must match a runner mapping `id`. |
| `mappings` | `source.database` | yes | CockroachDB database to stream from. |
| `mappings` | `source.tables` | yes | Tables to include in the changefeed. |

### Postgres-Grants configuration (`postgres-grants.yml`)

| Section | Field | Required | Description |
|---------|-------|----------|-------------|
| `mappings` | `id` | yes | Mapping identifier. |
| `mappings.destination` | `database` | yes | PostgreSQL database name. |
| `mappings.destination` | `runtime_role` | yes | Role that the runner will connect as. |
| `mappings.destination` | `tables` | yes | Tables the role needs `SELECT, INSERT, UPDATE, DELETE` on. |

## Important notes

- The runner creates a schema named `_cockroach_migration_tool` on each destination PostgreSQL database. Do not drop it while a migration is active.
- Shadow tables mirror the exact column layout of the destination tables (minus generated columns). Primary keys are used for conflict resolution.
- The reconcile loop runs inside a database transaction. If any table fails, the entire pass is rolled back and the error is recorded in the tracking tables.
- Webhooks must be reachable from the CockroachDB cluster. If you run the runner inside a private network, ensure the changefeed can reach the webhook listener.
