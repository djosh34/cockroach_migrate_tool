# Getting Started

This guide walks you through deploying the CockroachDB Migration Tool to replicate data from a CockroachDB cluster into PostgreSQL, with a verification service to confirm data consistency.

## Prerequisites

- Docker and Docker Compose
- A running CockroachDB source cluster with rangefeeds enabled
- A running PostgreSQL destination cluster
- TLS certificates for HTTPS webhook and optional database connections

## Quick Start with Docker Compose

The project ships three Docker Compose files under `artifacts/compose/`, one for each service. You run them together in a layered migration pipeline.

### 1. Generate CockroachDB Setup SQL

The `setup-sql` tool emits the SQL you need to run on CockroachDB to create changefeed webhooks. It does **not** connect to any database — it only reads a config and renders SQL to stdout.

Create a config file at `config/cockroach-setup.yml`:

```yaml
cockroach:
  url: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require
webhook:
  base_url: https://runner.example.internal:8443
  ca_cert_path: ca.crt
  resolved: 5s
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
```

Place your CA certificate at `config/ca.crt` (relative paths resolve from the config file's directory).

Run with Docker Compose:

```bash
SETUP_SQL_IMAGE=<your-registry>/setup-sql:latest \
docker compose -f artifacts/compose/setup-sql.compose.yml up
```

This emits CockroachDB changefeed creation SQL to stdout. Capture it and run it against your CockroachDB cluster:

```bash
docker compose -f artifacts/compose/setup-sql.compose.yml up > changefeed.sql
# Then apply against CockroachDB:
psql "postgresql://root@crdb:26257/defaultdb?sslmode=require" < changefeed.sql
```

The emitted SQL includes:

- `SET CLUSTER SETTING kv.rangefeed.enabled = true;`
- `SELECT cluster_logical_timestamp() AS changefeed_cursor;`
- `CREATE CHANGEFEED FOR TABLE ...` statements with `__CHANGEFEED_CURSOR__` placeholders

You must replace `__CHANGEFEED_CURSOR__` with the timestamp from the cursor query before executing the changefeeds.

You can also generate Postgres grants SQL separately:

```bash
setup-sql emit-postgres-grants --config config/postgres-grants.yml
```

With a config file like `config/postgres-grants.yml`:

```yaml
mappings:
  - id: app-a
    destination:
      database: app_a
      runtime_role: migration_user_a
      tables:
        - public.customers
        - public.orders
```

This emits `GRANT` statements for the runner's database role.

### 2. Start the Runner

The runner is the core service. It receives CDC events from CockroachDB changefeeds via HTTPS webhook and replicates them into PostgreSQL destination tables.

Create `config/runner.yml`:

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
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
      host: postgres
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
```

Run with Docker Compose (you need at minimum a server cert and key):

```bash
RUNNER_IMAGE=<your-registry>/runner:latest \
RUNNER_HTTPS_PORT=8443 \
docker compose -f artifacts/compose/runner.compose.yml up
```

For development/testing without TLS, use HTTP mode:

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
      url: postgresql://migration_user_a:secret@pg-a:5432/app_a
```

For mutual TLS (mTLS), add a `client_ca_path` under `tls` to require client certificates:

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

### 3. Start the Verify Service

The verify service compares data between source and destination to detect inconsistencies.

Create `config/verify-service.yml`:

```yaml
listener:
  bind_addr: "0.0.0.0:8080"
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
verify:
  source:
    url: postgres://root@crdb:26257/defaultdb?sslmode=require
  destination:
    url: postgres://migration_user@pg:5432/app_a?sslmode=require
  raw_table_output: true
```

Run with Docker Compose:

```bash
VERIFY_IMAGE=<your-registry>/verify:latest \
VERIFY_HTTPS_PORT=9443 \
docker compose -f artifacts/compose/verify.compose.yml up
```

Then start a verification job via the API:

```bash
curl -X POST https://localhost:9443/jobs \
  -H "Content-Type: application/json" \
  -d '{}'
```

Poll the job status:

```bash
curl https://localhost:9443/jobs/<job_id>
```

## Validating Configuration

Each binary supports a `validate-config` subcommand that checks the config without starting the service:

```bash
# Runner
runner validate-config --config config/runner.yml

# Runner with deep validation (connects to destination databases)
runner validate-config --config config/runner.yml --deep

# Setup SQL
setup-sql emit-cockroach-sql --config config/cockroach-setup.yml
# (emitting SQL implicitly validates the config)
```

## Configuration Reference

### Runner Config (`runner.yml`)

| Field | Required | Description |
|-------|----------|-------------|
| `webhook.bind_addr` | Yes | Address to bind the webhook listener (e.g. `0.0.0.0:8443`) |
| `webhook.mode` | No | `http` or `https` (default: `https`) |
| `webhook.tls.cert_path` | If `mode=https` | Path to the server TLS certificate |
| `webhook.tls.key_path` | If `mode=https` | Path to the server TLS private key |
| `webhook.tls.client_ca_path` | No | Path to the CA certificate for mTLS client verification |
| `reconcile.interval_secs` | Yes | Reconcile loop interval in seconds (must be > 0) |
| `mappings[].id` | Yes | Unique identifier for this mapping |
| `mappings[].source.database` | Yes | CockroachDB source database name |
| `mappings[].source.tables` | Yes | List of schema-qualified table names (e.g. `public.customers`) |
| `mappings[].destination.url` | One of `url` or decomposed fields | PostgreSQL connection URL |
| `mappings[].destination.host` | One of `url` or decomposed fields | PostgreSQL host |
| `mappings[].destination.port` | With decomposed | PostgreSQL port |
| `mappings[].destination.database` | With decomposed | PostgreSQL database name |
| `mappings[].destination.user` | With decomposed | PostgreSQL user |
| `mappings[].destination.password` | With decomposed | PostgreSQL password |
| `mappings[].destination.tls.mode` | No | `require`, `verify-ca`, or `verify-full` |
| `mappings[].destination.tls.ca_cert_path` | If `verify-ca` or `verify-full` | Path to CA certificate |
| `mappings[].destination.tls.client_cert_path` | No | Path to client certificate |
| `mappings[].destination.tls.client_key_path` | No | Path to client key |

**Constraints:**

- `url` and decomposed fields (`host`, `port`, etc.) are mutually exclusive for a destination
- Mapping IDs must be unique across the config
- Table names must be schema-qualified (e.g. `public.customers`, not just `customers`)
- Mappings sharing a destination database must use identical connection parameters
- No overlapping destination tables across mappings connected to the same database

### CockroachDB Setup Config (`cockroach-setup.yml`)

| Field | Required | Description |
|-------|----------|-------------|
| `cockroach.url` | Yes | CockroachDB connection URL (used only as a comment in emitted SQL) |
| `webhook.base_url` | Yes | Runner webhook base URL (must start with `https://`) |
| `webhook.ca_cert_path` | Yes | Path to the CA certificate for the changefeed webhook |
| `webhook.resolved` | Yes | Changefeed resolved timestamp interval (e.g. `5s`) |
| `mappings[].id` | Yes | Unique mapping identifier |
| `mappings[].source.database` | Yes | Source database name |
| `mappings[].source.tables` | Yes | Schema-qualified table names |

### Postgres Grants Config (`postgres-grants.yml`)

| Field | Required | Description |
|-------|----------|-------------|
| `mappings[].id` | Yes | Unique identifier (used for deduplication only) |
| `mappings[].destination.database` | Yes | Target PostgreSQL database |
| `mappings[].destination.runtime_role` | Yes | Role to receive grants |
| `mappings[].destination.tables` | Yes | Schema-qualified table names |

### Verify Service Config (`verify-service.yml`)

| Field | Required | Description |
|-------|----------|-------------|
| `listener.bind_addr` | Yes | Address to bind the HTTP(S) listener |
| `listener.tls.cert_path` | No | Server TLS certificate |
| `listener.tls.key_path` | No | Server TLS private key |
| `listener.tls.client_ca_path` | No | CA cert for mTLS client verification |
| `verify.source.url` | Yes | Source database connection URL |
| `verify.source.tls.*` | No | Source TLS settings (same structure as runner destination TLS) |
| `verify.destination.url` | Yes | Destination database connection URL |
| `verify.destination.tls.*` | No | Destination TLS settings |
| `verify.raw_table_output` | No | Enable the `POST /tables/raw` endpoint (default: disabled) |

## CLI Reference

### Runner

```
runner [OPTIONS] <COMMAND>

Options:
  --log-format <text|json>   Log output format (default: text)

Commands:
  validate-config   Validate configuration file
    --config <PATH>      Path to YAML config
    --deep               Also verify destination database schemas

  run               Start the runner service
    --config <PATH>      Path to YAML config
```

### Setup SQL

```
setup-sql [OPTIONS] <COMMAND>

Options:
  --log-format <text|json>   Log output format (default: text)

Commands:
  emit-cockroach-sql    Emit CockroachDB changefeed SQL
    --config <PATH>        Path to YAML config
    --format <text|json>   Output format (default: text)

  emit-postgres-grants  Emit PostgreSQL GRANT statements
    --config <PATH>        Path to YAML config
    --format <text|json>   Output format (default: text)
```

### Verify Service

```
molt verify-service [OPTIONS] <COMMAND>

Options:
  --log-format <text|json>   Log output format (default: text)

Commands:
  validate-config   Validate configuration file
    --config <PATH>      Path to YAML config

  run               Start the verify HTTP service
    --config <PATH>      Path to YAML config
```

The verify service can also be used as a CLI tool:

```
molt verify --source <url> --target <url> [flags]

Flags:
  --concurrency <int>        Number of parallel workers
  --row-batch-size <int>     Rows per batch
  --table-splits <int>        Number of shards per table
  --live                      Retry mismatches with exponential backoff
  --continuous                Run in a loop
  --fixup                     Auto-fix detected mismatches
```

## API Endpoints (Verify Service)

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/jobs` | Start a verification job |
| `GET` | `/jobs/{id}` | Poll job status and results |
| `POST` | `/jobs/{id}/stop` | Cancel a running job |
| `POST` | `/tables/raw` | Read raw table data (if enabled) |
| `GET` | `/metrics` | Prometheus metrics |
| `GET` | `/healthz` | Health check (runner and verify) |

The runner also exposes `/metrics` and `/healthz` on its webhook listener.