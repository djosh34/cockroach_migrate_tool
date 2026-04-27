# Architecture

## Migration Goal

The project moves selected CockroachDB source tables into PostgreSQL-compatible destination databases. It is built around a shadow-table pipeline:

1. Operators prepare destination schemas and grants.
2. Operators generate CockroachDB `CREATE CHANGEFEED` SQL with `setup-sql`.
3. CockroachDB sends enriched webhook events to the `runner`.
4. The `runner` writes incoming source changes into destination-side shadow tables in `_cockroach_migration_tool`.
5. A periodic reconcile loop applies shadow table state into the real destination tables.
6. The verify service runs Molt verification jobs to compare source and destination data.

The design separates source setup, live ingestion, destination reconciliation, and verification. The source changefeed only needs to reach the runner webhook. Destination mutation is performed by the runner using its configured PostgreSQL credentials.

## High-Level Flow

```text
CockroachDB source
  |
  | CREATE CHANGEFEED ... INTO 'webhook-https://runner/ingest/{mapping_id}?ca_cert=...'
  v
runner webhook runtime
  |
  | parse enriched event, route by mapping, table, and database
  v
_cockroach_migration_tool.<mapping>__<schema>__<table> shadow tables
  |
  | periodic FK-aware reconcile
  v
destination real tables
  |
  | on-demand verify job
  v
verify-service / Molt verifier
```

A mapping is the main unit of migration. Each mapping has:

- A unique `id`.
- One source database.
- One or more schema-qualified source tables.
- One PostgreSQL-compatible destination database connection.

Mappings targeting the same destination host, port, and database are grouped together at runner startup. Within a group, the full destination connection contract must be consistent, and two mappings cannot own the same destination table.

## Workspace Components

| Component | Language | Purpose |
| --- | --- | --- |
| `crates/setup-sql` | Rust | Emits source CockroachDB bootstrap SQL and destination grants SQL. |
| `crates/runner` | Rust | Runs webhook ingestion, shadow table persistence, reconcile, tracking, and metrics. |
| `crates/ingest-contract` | Rust | Shared ingest path rendering contract, currently `/ingest/{mapping_id}`. |
| `crates/operator-log` | Rust | Shared text and JSON operator log event formatting. |
| `cockroachdb_molt/molt` | Go | Vendored Molt code plus the verify-service command and HTTP API. |
| `artifacts/compose` | YAML | Thin Compose wrappers for runner, setup-sql, and verify-service images. |
| `openapi/verify-service.yaml` | YAML | HTTP contract for verify-service clients. |

## `setup-sql`

`setup-sql` is a one-time SQL emission CLI. It never connects to databases. It reads YAML, validates it, and writes SQL or JSON to stdout.

Commands:

```sh
setup-sql emit-cockroach-sql --config <path> [--format text|json]
setup-sql emit-postgres-grants --config <path> [--format text|json]
```

### Cockroach SQL Emission

Input shape:

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

Validation rules from the parser:

- `cockroach.url`, `webhook.base_url`, `webhook.ca_cert_path`, `webhook.resolved`, mapping IDs, source databases, and table names must be non-empty after trimming.
- `webhook.base_url` must start with `https://`.
- `webhook.ca_cert_path` may be relative to the config file directory.
- `mappings` must contain at least one mapping.
- Mapping IDs must be unique.
- Each mapping must list at least one source table.
- Table names must be schema-qualified.
- Duplicate tables inside one mapping are rejected.

Rendering behavior:

- Mappings are grouped by source database.
- The emitted SQL starts with `SET CLUSTER SETTING kv.rangefeed.enabled = true;`.
- The emitted SQL includes `SELECT cluster_logical_timestamp() AS changefeed_cursor;`.
- Each mapping gets a `CREATE CHANGEFEED FOR TABLE ... INTO ...` statement.
- The sink URL uses `webhook-<base_url>/ingest/<mapping_id>?ca_cert=<base64-url-escaped-ca>`.
- Changefeeds are created with `initial_scan = 'yes'`, `envelope = 'enriched'`, `enriched_properties = 'source'`, and the configured `resolved` interval.
- The cursor placeholder is `__CHANGEFEED_CURSOR__`; operators replace it with the captured logical timestamp.

### PostgreSQL Grants Emission

Input shape:

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

Validation rules:

- `mappings` must contain at least one mapping.
- Mapping IDs must be unique.
- Destination database and runtime role must be non-empty.
- Destination table lists must be non-empty.
- Destination tables must be schema-qualified and unique within a mapping.

Rendering behavior:

- Grants are grouped and deduplicated by destination database.
- The generated statements grant `CONNECT, CREATE` on the database.
- The generated statements grant `USAGE` on schema `public`.
- The generated statements grant `SELECT, INSERT, UPDATE, DELETE` on each configured table.

The helper schema name is fixed as `_cockroach_migration_tool`.

## `runner`

`runner` is the long-running migration runtime.

Commands:

```sh
runner validate-config --config <path> [--deep]
runner run --config <path>
```

`validate-config` parses and validates YAML. With `--deep`, it also opens destination connections and loads configured table catalog metadata.

`run` performs startup bootstrap and then runs two runtimes concurrently:

- Webhook runtime.
- Reconcile runtime.

If either runtime returns an error, the process fails.

### Runner Config Model

Top-level YAML fields:

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
    destination:
      host: postgres
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
```

Config parser behavior:

- Unknown fields are denied.
- `webhook.bind_addr` must parse as a socket address.
- `webhook.mode` defaults to `https`.
- `webhook.mode: http` rejects any `webhook.tls`.
- `webhook.mode: https` requires `webhook.tls`.
- HTTPS requires server cert and key paths.
- `webhook.tls.client_ca_path` enables mTLS.
- `reconcile.interval_secs` must be greater than zero.
- `mappings` must contain at least one mapping.
- Mapping IDs must be unique.
- Source database names and table names must be non-empty.
- Source table names must be schema-qualified as `schema.table`.
- Destination config can use either `url` or decomposed fields, not both.
- Destination Unix sockets are rejected; destinations must use TCP hosts.
- Destination database must be present.

Destination TLS rules:

- URL mode relies on URL parameters accepted by `sqlx`/PostgreSQL connection parsing.
- Decomposed mode supports `mode: require`, `mode: verify-ca`, and `mode: verify-full`.
- `verify-ca` and `verify-full` require `ca_cert_path`.
- `client_cert_path` and `client_key_path` must be set together.

### Startup Plan

Runner startup converts validated config into a `RunnerStartupPlan`:

- Converts source table names into quoted SQL identifiers.
- Groups mappings by destination host, port, and database.
- Rejects inconsistent destination connection contracts inside one destination group.
- Rejects overlapping destination table ownership inside one destination group.
- Stores webhook listener transport and reconcile interval.

Deep validation and startup bootstrap both use destination catalog loading.

### Destination Catalog Loading

For every configured destination table, the runner reads PostgreSQL catalog metadata:

- Columns from `pg_attribute`, including raw formatted type, nullability, and generated-column flag.
- Primary-key columns from `pg_constraint`.
- Foreign keys from `pg_constraint`, including referenced table and delete action.

Missing tables fail validation/startup. Unsupported foreign key delete action metadata also fails.

This catalog data drives:

- Shadow table DDL.
- Primary-key indexes on shadow tables.
- Reconcile ordering.
- Upsert and delete SQL generation.

### Destination Bootstrap

On `runner run`, before serving traffic, the runner connects to each destination group and creates helper structures:

```sql
CREATE SCHEMA IF NOT EXISTS _cockroach_migration_tool;
```

It creates `_cockroach_migration_tool.stream_state`:

```text
mapping_id text primary key
source_database text not null
source_job_id text
starting_cursor text
latest_received_resolved_watermark text
latest_reconciled_resolved_watermark text
stream_status text not null default 'bootstrap_pending'
```

It creates `_cockroach_migration_tool.table_sync_state`:

```text
mapping_id text not null
source_table_name text not null
helper_table_name text not null
last_successful_sync_time timestamptz
last_successful_sync_watermark text
last_error text
primary key (mapping_id, source_table_name)
```

For each mapped destination table, it creates a shadow table:

```text
_cockroach_migration_tool.<mapping_id>__<schema>__<table>
```

The shadow table uses the destination table column names, raw PostgreSQL types, and nullability. If the destination table has a primary key, the runner creates a unique index on the shadow table primary-key columns.

Startup also seeds tracking rows for each mapping and table. Existing tracking rows are updated where needed rather than duplicated.

### Webhook Runtime

The webhook runtime binds the configured socket and serves:

- `GET /healthz`
- `GET /metrics`
- `POST /ingest/{mapping_id}`

HTTP mode uses Axum directly. HTTPS mode uses Rustls. If `client_ca_path` is configured, the listener requires and verifies client certificates.

TLS details:

- Server certificates and private keys are read from PEM files.
- At least one server certificate and one private key must be present.
- mTLS loads client CA certificates into a Rustls root store.
- ALPN includes `h2` and `http/1.1`.
- TLS handshake failures are logged as operator errors and do not crash the listener.

### Webhook Payload Contract

The runner accepts two Cockroach webhook shapes.

Resolved timestamp:

```json
{
  "resolved": "1234567890.0000000000"
}
```

Row batch:

```json
{
  "length": 1,
  "payload": [
    {
      "op": "c",
      "source": {
        "database_name": "demo_a",
        "schema_name": "public",
        "table_name": "customers"
      },
      "key": {
        "id": 101
      },
      "after": {
        "id": 101,
        "name": "Ada"
      }
    }
  ]
}
```

Payload parser behavior:

- Body must be valid JSON object.
- A non-empty string `resolved` field makes the request a resolved-watermark request.
- Row batches must contain `payload` array and `length`.
- `length` must equal `payload.length`.
- Empty payload arrays are rejected.
- Operations `c`, `u`, and `r` are upserts.
- Operation `d` is a delete.
- Upserts require `after`.
- All row events require `source`, `key`, and source fields `database_name`, `schema_name`, and `table_name`.

Routing rules:

- The path mapping ID must exist.
- Every row must match the configured source database.
- Every row must target a mapped source table.
- A row batch may contain rows for only one source table.
- Resolved events update mapping-level tracking state.

Bad payload or routing errors return `400`. Persistence failures return `500`.

### Shadow Persistence

For each row batch, the runner:

1. Opens a PostgreSQL connection to the destination.
2. Starts a transaction.
3. Applies each mutation to the selected shadow table.
4. Commits the transaction.

Upsert persistence uses `jsonb_populate_record` to cast JSON row data into the shadow table type. If the shadow table has primary-key columns, inserts use `ON CONFLICT (...) DO UPDATE SET ...`. Without primary-key columns, upserts are plain inserts.

Delete persistence requires primary-key columns. It converts the changefeed key JSON into a record and deletes shadow rows where primary-key columns are `IS NOT DISTINCT FROM` the key values.

Resolved watermark persistence updates `stream_state.latest_received_resolved_watermark` only when the incoming watermark is newer than the stored value.

### Reconcile Runtime

The reconcile runtime creates one loop per mapping. Each loop waits `reconcile.interval_secs`, then runs a reconcile pass forever.

Each reconcile pass:

1. Opens a destination connection.
2. Starts a transaction.
3. Applies shadow-to-real upserts in dependency order.
4. Applies real-table deletes in reverse dependency order.
5. Persists reconcile success tracking.
6. Commits.
7. Refreshes table metrics.

If applying a table fails:

1. The transaction is rolled back.
2. Failure details are written to `table_sync_state.last_error`.
3. An operator error event is logged.
4. Failure metrics are updated.
5. Table metrics are refreshed.

Foreign-key dependency order is derived from destination catalog metadata among the selected tables. Parents are upserted before children. Deletes happen in reverse order. Cycles among selected tables fail helper-plan creation.

### Reconcile Apply SQL

The runner has separate upsert and delete modules.

Upsert reconcile copies rows from the shadow table into the real table. Generated destination columns are tracked in the helper plan and are not treated like normal mutable input columns.

Delete reconcile removes real rows that no longer exist in shadow state. Deletes require primary-key metadata.

Both phases run inside the same transaction for a mapping pass.

### Metrics

Runner metrics are emitted in Prometheus text format from `GET /metrics`. Metric names use the prefix:

```text
cockroach_migration_tool_
```

Metric groups include:

- Webhook request counts by destination database, payload kind, and outcome.
- Last webhook request timestamp.
- Webhook apply durations and request counts by destination table.
- Reconcile apply durations and attempt counts by phase.
- Shadow and real table row counts.
- Table reconcile error state.
- Last successful reconcile timestamp.
- Apply failure counts and last outcome timestamps.

Metrics are held in process memory and refreshed during webhook handling and reconcile passes.

## `operator-log`

`operator-log` provides the shared Rust log format.

`--log-format text` writes only the event message.

`--log-format json` writes structured JSON fields:

```json
{
  "timestamp": "2026-04-27T12:00:00Z",
  "level": "info",
  "service": "runner",
  "event": "config.validated",
  "message": "runner config validated"
}
```

Commands write command results and failures through this event model. JSON mode is intended for operator automation.

## `ingest-contract`

`ingest-contract` owns the shared URL path for a mapping ingest endpoint:

```text
/ingest/{mapping_id}
```

`setup-sql` uses it while rendering Cockroach changefeed sink URLs. `runner` serves the same route.

## Verify Service

The verify service is implemented in the vendored Go Molt subtree and exposed through:

```sh
molt verify-service validate-config --config <path>
molt verify-service run --config <path>
```

The Docker image entrypoint is:

```text
/usr/local/bin/molt verify-service
```

### Verify Config

Config shape:

```yaml
listener:
  bind_addr: 0.0.0.0:8080
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
    url: postgresql://verify_target@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
      client_cert_path: /config/certs/destination-client.crt
      client_key_path: /config/certs/destination-client.key
  raw_table_output: true
```

Validation behavior:

- Unknown YAML fields are rejected.
- `listener.bind_addr` is required.
- Listener TLS requires both cert and key paths.
- `listener.tls.client_ca_path` enables HTTPS mTLS.
- Source and destination URLs must use `postgres` or `postgresql` scheme.
- If a database URL has `sslmode=verify-ca` or `sslmode=verify-full`, the corresponding TLS `ca_cert_path` is required.
- Database client cert and key paths must be set together.

When connecting to source and destination, TLS file paths from `verify.*.tls` are inserted into the PostgreSQL URL as `sslrootcert`, `sslcert`, and `sslkey`.

### Verify HTTP API

The service exposes:

- `POST /jobs`: start a verify job.
- `GET /jobs/{job_id}`: fetch active or most recent completed job.
- `POST /jobs/{job_id}/stop`: request cancellation.
- `POST /tables/raw`: read raw table rows when enabled.
- `GET /metrics`: Prometheus metrics.

Only one verify job can run at a time. The service keeps only the active job and the most recent completed job in memory. Restarting the process loses retained job state.

Job request filters are top-level POSIX regular expressions:

```json
{
  "include_schema": "^public$",
  "include_table": "^(customers|orders)$",
  "exclude_schema": "^audit$",
  "exclude_table": "^tmp_"
}
```

Empty include filters default to Molt's default include-all filter. Invalid regex filters return a request validation error.

Raw table output request:

```json
{
  "database": "destination",
  "schema": "public",
  "table": "customers"
}
```

`database` must be `source` or `destination`. `schema` and `table` must match a simple identifier pattern: start with a letter or underscore, followed by letters, digits, or underscores.

### Verify Runtime

The verify runtime creates an HTTP server using the configured listener. If listener TLS is present, it loads the server certificate and key. If a client CA is configured, it requires and verifies client certificates.

A started job runs in a background goroutine. Results are reported back into the service state. A job can finish as:

- `succeeded`
- `failed`
- `stopped`

Mismatches are classified as failures even if the underlying verification command returned normally. Operator errors redact credentials embedded in URLs before they are returned or logged.

## Docker And Compose Contracts

The Dockerfiles are production-style multi-stage builds:

- Rust images use `cargo-chef`, `--locked`, release mode, and musl targets.
- The Go verify image uses `CGO_ENABLED=0`, `GOOS=linux`, and strips symbols.
- Runtime stages are `scratch`.

Compose files are intentionally minimal:

- `runner.compose.yml` runs `runner run --log-format json --config /config/runner.yml`.
- `setup-sql.compose.yml` runs `setup-sql emit-cockroach-sql --log-format json --config /config/cockroach-setup.yml`.
- `verify.compose.yml` runs `molt verify-service run --log-format json --config /config/verify-service.yml`.

The compose artifacts assume local `./config` files and image names supplied through environment variables.

## Failure Model

The implementation generally fails closed:

- Config parsers reject missing required fields and unknown fields where configured.
- Startup fails when destination metadata cannot be loaded.
- Webhook payload and routing errors return explicit `400` responses.
- Destination persistence errors return `500` responses.
- Reconcile apply failures are recorded in tracking state and metrics.
- Verify service returns structured operator error envelopes.
- JSON log modes emit structured failure events for command automation.

No component silently ignores connection, parsing, catalog, TLS, or persistence errors.

