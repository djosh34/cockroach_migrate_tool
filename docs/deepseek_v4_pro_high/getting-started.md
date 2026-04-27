# Getting Started

This guide walks you through setting up the CockroachDB-to-PostgreSQL migration tool end-to-end.

## Overview

The tool replicates data from CockroachDB source databases to PostgreSQL destinations using CockroachDB's Change Data Capture (CDC) changefeeds. It consists of three binaries:

| Binary | Role |
|---|---|
| **setup-sql** | One-time CLI that emits SQL for configuring CockroachDB changefeeds and PostgreSQL permissions |
| **runner** | Long-running daemon that receives changefeed events and applies them to PostgreSQL |
| **verify** | On-demand HTTP API for comparing source vs destination data consistency |

## Prerequisites

- A **source CockroachDB cluster** with `kv.rangefeed.enabled` and changefeed support
- A **destination PostgreSQL server** (v12+) with sufficient permissions to create schemas and tables
- A machine with **Docker Compose** to run the components
- **TLS certificates** for secure communication between all components

## File Layout

Create a workspace directory with this structure:

```
workspace/
├── config/
│   ├── runner.yml                  # Runner configuration
│   ├── cockroach-setup.yml         # setup-sql CockroachDB config
│   ├── postgres-grants.yml         # setup-sql PostgreSQL grants config
│   ├── verify-service.yml          # Verify service config
│   ├── ca.crt                      # CA certificate for CockroachDB→runner trust
│   └── certs/
│       ├── server.crt              # Runner's server certificate
│       └── server.key              # Runner's server private key
├── setup-sql.compose.yml           # From artifacts/compose/
├── runner.compose.yml              # From artifacts/compose/
└── verify.compose.yml              # From artifacts/compose/
```

## Step 1: Write Configuration Files

### Runner Config (`config/runner.yml`)

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
      host: pg.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
    destination:
      host: pg.example.internal
      port: 5432
      database: app_b
      user: migration_user_b
      password: runner-secret-b
```

**Key fields:**

- `webhook.bind_addr` — Address the HTTP(S) server listens on
- `webhook.mode` — `http` or `https` (default: `https`; if `https`, `tls` is required)
- `webhook.tls` — Server certificate, key, and optional `client_ca_path` for mTLS
- `reconcile.interval_secs` — How often the runner syncs shadow tables to real tables
- `mappings[].id` — Unique identifier linking source tables to a destination
- `mappings[].source.database` — CockroachDB database name (informational)
- `mappings[].source.tables` — Fully-qualified table names (`schema.table`)
- `mappings[].destination` — PostgreSQL connection: either a `url` string or decomposed `host`/`port`/`database`/`user`/`password` fields

**TLS for PostgreSQL connections** (optional):

```yaml
    destination:
      host: pg.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
      tls:
        mode: verify-ca
        ca_cert_path: /config/certs/pg-ca.crt
```

### CockroachDB Setup Config (`config/cockroach-setup.yml`)

```yaml
cockroach:
  url: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require
webhook:
  base_url: https://runner.example.internal:8443
  ca_cert_path: /config/ca.crt
  resolved: 5s
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
```

**Key fields:**

- `cockroach.url` — Connection URL to the CockroachDB source cluster
- `webhook.base_url` — Public URL of the runner's webhook endpoint (must start with `https://`)
- `webhook.ca_cert_path` — Path to the CA cert that CockroachDB will use to trust the runner's server certificate
- `webhook.resolved` — Changefeed resolved timestamp interval (e.g., `5s`, `30s`, `1m`)

### PostgreSQL Grants Config (`config/postgres-grants.yml`)

```yaml
mappings:
  - id: app-a
    destination:
      database: app_a
      runtime_role: migration_user_a
      tables:
        - public.customers
        - public.orders
  - id: app-b
    destination:
      database: app_b
      runtime_role: migration_user_b
      tables:
        - public.invoices
```

### Verify Service Config (`config/verify-service.yml`)

```yaml
listener:
  bind_addr: 0.0.0.0:8080
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
source:
  conn_string: postgresql://reader@crdb.example.internal:26257/demo_a?sslmode=verify-ca&sslrootcert=/config/certs/source-ca.crt&sslcert=/config/certs/source-client.crt&sslkey=/config/certs/source-client.key
destination:
  conn_string: postgresql://reader@pg.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt
```

## Step 2: Prepare Docker Compose Files

Copy the compose files from the `artifacts/compose/` directory to your workspace. They reference config files and certs as Docker Compose `configs:` resources.

## Step 3: Generate and Run Setup SQL

**Generate CockroachDB changefeed SQL:**

```bash
docker compose -f setup-sql.compose.yml run --rm setup-sql emit-cockroach-sql \
  --log-format text --config /config/cockroach-setup.yml
```

This outputs SQL you must run on the CockroachDB cluster:
1. `SET CLUSTER SETTING kv.rangefeed.enabled = true;`
2. `SELECT cluster_logical_timestamp() AS changefeed_cursor;` — Capture the returned cursor value
3. `CREATE CHANGEFEED FOR TABLE ...` — Replace `__CHANGEFEED_CURSOR__` with the captured cursor

**Generate PostgreSQL grant SQL:**

```bash
docker compose -f setup-sql.compose.yml run --rm setup-sql emit-postgres-grants \
  --log-format text --config /config/postgres-grants.yml
```

Run the resulting `GRANT` statements on your destination PostgreSQL server.

## Step 4: Start the Runner

```bash
RUNNER_IMAGE=quay.io/org/runner:latest docker compose -f runner.compose.yml up -d
```

The runner will:
1. Connect to each destination PostgreSQL database
2. Create the `_cockroach_migration_tool` schema with tracking tables
3. Create shadow tables mirroring source table schemas
4. Start the HTTPS webhook server
5. Begin the reconcile loop

**Validate a config without running:**

```bash
docker compose -f runner.compose.yml run --rm runner validate-config --config /config/runner.yml
```

Add `--deep` to actually connect to the destination databases and validate schemas.

## Step 5: Monitor

The runner exposes Prometheus metrics at `GET /metrics` on the same port:

```bash
curl -s https://runner.example.internal:8443/metrics
```

Key metrics include:
- Webhook request counts by kind (row_batch, resolved) and outcome (ok, bad_request, internal_error)
- Reconcile apply attempts and durations per mapping
- Row counts in shadow tables vs real tables
- Reconcile errors
- Last successful reconcile timestamp

A health endpoint is available at `GET /healthz`.

## Step 6: Run Data Verification

```bash
VERIFY_IMAGE=quay.io/org/verify:latest docker compose -f verify.compose.yml up -d
```

Trigger a verification job:

```bash
curl -X POST https://verify.example.internal:9443/jobs \
  -H "Content-Type: application/json" \
  -d '{"table_filter": ".*", "schema_filter": "public", "live": false, "continuous": false}'
```

Check job status:

```bash
curl -s https://verify.example.internal:9443/jobs/<job-id> | jq .
```

## Output Formats

Both `setup-sql` and `runner` support `--log-format text` (default) and `--log-format json`. The `setup-sql` tool also supports `--format json` for structured SQL output.

Use JSON log format in production:

```bash
runner run --log-format json --config /config/runner.yml
```
