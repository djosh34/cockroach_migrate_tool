# Architecture

## Migration Overview

The CockroachDB Migration Tool replicates data from CockroachDB source tables into PostgreSQL destination tables using CockroachDB's Change Data Capture (CDC) changefeed webhook mechanism. It is not a one-time dump-and-load tool — it is a continuous, eventually-consistent replication system that can run indefinitely.

The migration has three distinct phases:

### Phase 1: Bootstrap (Setup SQL)

Before replication can begin, CockroachDB must be configured to stream changes. The `setup-sql` binary generates the SQL statements needed:

1. **Enable rangefeeds** on the CockroachDB cluster (`SET CLUSTER SETTING kv.rangefeed.enabled = true`)
2. **Capture a cursor** — a logical timestamp that marks the starting point for changefeeds
3. **Create changefeeds** — one changefeed per mapping, which delivers every insert, update, and delete as a JSON webhook payload to the runner's `/ingest/{mapping_id}` endpoint
4. **Grant permissions** on the PostgreSQL destination — the runner needs `SELECT`, `INSERT`, `UPDATE`, `DELETE` on each target table

The `setup-sql` tool does **not** connect to any database. It reads a YAML config and renders SQL to stdout. An operator must manually apply this SQL against the respective databases, replacing the `__CHANGEFEED_CURSOR__` placeholder with the actual cursor timestamp.

### Phase 2: Continuous Replication (Runner)

The `runner` binary is the heart of the system. It runs two concurrent processes:

1. **Webhook receiver** — an HTTP/HTTPS server that receives CDC events from CockroachDB changefeeds
2. **Reconcile loop** — a periodic background process that copies data from shadow tables into real destination tables

The flow for each data mutation is:

```
CockroachDB changefeed
        |
        v
 POST /ingest/{mapping_id}  (webhook)
        |
        v
 Parse JSON payload (RowBatch or Resolved)
        |
        v
 Route to correct destination + shadow table
        |
        v
 Persist to shadow table in _cockroach_migration_tool schema
 (or update resolved watermark)
        |
        v  [periodic reconcile loop]
 Copy from shadow tables to real destination tables
 (upserts in FK dependency order, deletes in reverse)
```

The **shadow table** pattern is the key design decision. CDC events are first written to helper tables in a dedicated `_cockroach_migration_tool` schema, not directly to the real destination tables. A separate reconcile loop then applies changes from shadow to real tables in the correct foreign-key order. This separation provides:

- **Atomicity** — each reconcile pass runs in a single transaction
- **Ordering guarantees** — upserts respect FK parent-before-child; deletes go child-before-parent
- **Resilience** — a failed reconcile pass rolls back without losing the shadow data; the next pass retries
- **Observability** — shadow tables can be inspected directly, and metrics track shadow vs real row counts

### Phase 3: Verification (Verify Service)

The `molt verify-service` binary provides independent verification that source and destination data match. It connects to both databases and performs:

1. **Database-level verification** — discovers all user tables in both databases, reports missing or extraneous tables
2. **Table-level verification** — compares column names, types, nullability, and primary keys between matching tables
3. **Row-level verification** — for tables with matching schemas, performs a sorted merge-join comparison of all rows, detecting missing, extraneous, and mismatching values
4. **Live reverification** — optionally retries mismatches with exponential backoff to account for replication lag

The verify service runs as an HTTP API, accepting verification jobs and returning structured results with per-table row counts and detailed mismatch reports.

---

## Component Architecture

### Project Layout

```
cockroach_migrate_tool/
  Cargo.toml                  # Rust workspace (runner, setup-sql, ingest-contract, operator-log)
  Dockerfile                  # Runner container image
  crates/
    runner/                   # Core replication runtime (Rust)
    setup-sql/                # SQL emission CLI (Rust)
    ingest-contract/          # URL path contract library (Rust)
    operator-log/             # Structured logging library (Rust)
  cockroachdb_molt/molt/     # Verification service (Go)
  openapi/
    verify-service.yaml       # OpenAPI spec for verify service
  artifacts/compose/          # Docker Compose files
```

### Dependency Graph

```
operator-log (leaf)
     ^
     |
  runner            setup-sql
     ^                 ^
     |                 |
     +-- ingest-contract --+
              ^
              |  (shared URL path contract)
              |
     Both runner and setup-sql depend on the
     /ingest/{mapping_id} path convention from
     ingest-contract (setup-sql uses it actively;
     runner lists it in Cargo.toml for future use)

molt (verify) — standalone Go binary, no Rust dependencies
```

---

## Component Deep Dive: Runner

The runner is a long-running Rust binary that orchestrates continuous data replication.

### CLI Interface

```
runner [--log-format <text|json>] <COMMAND>

Commands:
  validate-config --config <PATH> [--deep]
  run --config <PATH>
```

- `validate-config` — loads and validates the YAML config. With `--deep`, also connects to each destination database to verify table schemas exist.
- `run` — starts the webhook server and reconcile loop. Runs indefinitely.

### Startup Flow

```
Load YAML config
  -> Parse RawRunnerConfig from YAML
  -> Validate fields (unique IDs, schema-qualified tables, mutually exclusive fields, TLS constraints)
  -> Build RunnerConfig

Build RunnerStartupPlan
  -> Group mappings by destination database
  -> Detect conflicting connection configs for shared databases
  -> Detect overlapping destination tables

Bootstrap Postgres (for each destination group)
  -> Connect to destination database
  -> CREATE SCHEMA IF NOT EXISTS _cockroach_migration_tool
  -> CREATE TABLE _cockroach_migration_tool.stream_state (...)
  -> CREATE TABLE _cockroach_migration_tool.table_sync_state (...)
  -> For each mapped table:
      -> Introspect destination table schema (columns, PKs, FKs)
      -> Build shadow table DDL (columns mirror the real table)
      -> CREATE TABLE _cockroach_migration_tool.{mapping_id}__{schema}__{table} (...)
      -> CREATE UNIQUE INDEX ... ON ... (primary_key_columns)
      -> Seed tracking state row

Build RunnerRuntimePlan
  -> Merge startup plan + bootstrap results (helper table plans, reconcile orders)

Concurrent execution:
  -> tokio::try_join!(serve_webhook_runtime, serve_reconcile_runtime)
```

### Tracking State Schema

The runner maintains two tracking tables in the `_cockroach_migration_tool` schema:

**`stream_state`** — one row per mapping:

| Column | Type | Description |
|--------|------|-------------|
| `mapping_id` | TEXT PK | Mapping identifier |
| `source_database` | TEXT NOT NULL | Source CockroachDB database |
| `source_job_id` | TEXT | Changefeed job ID |
| `starting_cursor` | TEXT | Initial changefeed cursor |
| `latest_received_resolved_watermark` | TEXT | Highest resolved timestamp received |
| `latest_reconciled_resolved_watermark` | TEXT | Highest watermark successfully reconciled |
| `stream_status` | TEXT | Default: `bootstrap_pending` |

**`table_sync_state`** — one row per mapping + source table:

| Column | Type | Description |
|--------|------|-------------|
| `mapping_id` | TEXT | Mapping identifier |
| `source_table_name` | TEXT | Source table name |
| `helper_table_name` | TEXT | Shadow table name |
| `last_successful_sync_time` | TIMESTAMPTZ | Last successful reconcile |
| `last_successful_sync_watermark` | TEXT | Watermark at last reconcile |
| `last_error` | TEXT | Error message if last reconcile failed |

### Shadow Table Pattern

For each mapped source table (e.g. `public.customers`), the runner creates a shadow table named `{mapping_id}__{schema}__{table}` (e.g. `app_a__public__customers`) in the `_cockroach_migration_tool` schema. This shadow table mirrors the destination table's column definitions exactly (type and nullability).

A unique index on the primary key columns ensures fast upsert and delete operations.

### Webhook Ingest Pipeline

The webhook server routes three paths:

| Path | Description |
|------|-------------|
| `GET /healthz` | Health check, returns `"ok"` |
| `GET /metrics` | Prometheus metrics in text format |
| `POST /ingest/{mapping_id}` | CDC event ingestion |

When a changefeed delivers a batch to `/ingest/{mapping_id}`, the runner:

1. **Parses** the JSON payload into `WebhookRequest` — either a `RowBatch` (array of row events) or a `Resolved` (watermark marker)
2. **Routes** the request to the correct destination and table
3. **Validates** that a row batch contains events for exactly one source table
4. **Persists** row mutations to the shadow table using `jsonb_populate_record` for efficient batch operations
5. **Records** resolved watermarks in `stream_state`

Row events carry an `op` field:
- `c` (insert) or `u` (update) or `r` (bootstrap row) → upsert into shadow table
- `d` (delete) → delete from shadow table

The persistence SQL uses PostgreSQL's `jsonb_populate_record` to cast JSON values to the correct column types, then performs `INSERT ... ON CONFLICT DO UPDATE` for upserts and `DELETE ... USING jsonb_populate_record ... WHERE IS NOT DISTINCT FROM` for deletes.

### Reconcile Loop

For each mapping, the runner spawns a periodic reconcile task that runs every `interval_secs`:

1. **Open** a transaction on the destination database
2. **Upsert phase** — for each table in topological (FK) order:
   ```sql
   INSERT INTO "real_schema"."real_table" ("col1", "col2", ...)
   SELECT helper."col1", helper."col2", ...
   FROM _cockroach_migration_tool."helper_table" AS helper
   ON CONFLICT ("pk1", "pk2") DO UPDATE SET "col1" = EXCLUDED."col1", ...
   ```
   Generated columns are excluded. If only PK columns exist, uses `ON CONFLICT DO NOTHING`.
3. **Delete phase** — for each table in reverse topological order:
   ```sql
   DELETE FROM "real_schema"."real_table" AS target
   WHERE NOT EXISTS (
     SELECT 1 FROM _cockroach_migration_tool."helper_table" AS helper
     WHERE helper."pk1" IS NOT DISTINCT FROM target."pk1"
   )
   ```
4. **Commit** the transaction
5. **On success**: update `stream_state.latest_reconciled_resolved_watermark` (monotonic max with received watermark), clear `last_error`, set `last_successful_sync_time = NOW()`
6. **On failure**: roll back the transaction, record the error in `table_sync_state.last_error`

### Topological Ordering (FK Dependencies)

Tables are ordered using Kahn's algorithm on the foreign-key DAG:

- **Upsert order**: parents before children (so FK constraints are satisfied)
- **Delete order**: reverse of upsert order (children before parents)
- **Cycles** are detected and reported as configuration errors

### TLS Support

The webhook server supports three modes:

| Mode | Behavior |
|------|----------|
| `http` | Plain HTTP on the bound address |
| `https` | TLS with `rustls` / `ring`, serves HTTP/1.1 and HTTP/2 via ALPN |
| `https` + `client_ca_path` | Mutual TLS — clients must present a certificate signed by the configured CA |

Destination Postgres connections support three TLS modes:

| Mode | Behavior |
|------|----------|
| `require` | TLS required, no certificate verification |
| `verify-ca` | TLS required, server CA certificate verified |
| `verify-full` | TLS required, server CA certificate and hostname verified |

### Metrics

The runner exposes Prometheus metrics at `GET /metrics`:

| Metric | Description |
|--------|-------------|
| `cockroach_migration_tool_webhook_requests_total{kind, outcome}` | Webhook request counts by type (row_batch/resolved) and outcome (success/error) |
| `cockroach_migration_tool_webhook_apply_duration_seconds{kind}` | Webhook payload apply duration |
| `cockroach_migration_tool_reconcile_apply_duration_seconds` | Reconcile pass duration |
| `cockroach_migration_tool_reconcile_attempts_total{outcome}` | Reconcile attempt counts by outcome |
| `cockroach_migration_tool_shadow_row_count{mapping_id, table}` | Row count in shadow tables |
| `cockroach_migration_tool_real_row_count{mapping_id, table}` | Row count in real tables |
| `cockroach_migration_tool_reconcile_error_flag{mapping_id, table}` | Whether a table's last reconcile failed |
| `cockroach_migration_tool_last_successful_sync_timestamp{mapping_id, table}` | Timestamp of last successful reconcile per table |
| `cockroach_migration_tool_webhook_apply_failures_total{kind, stage}` | Webhook apply failure counts |

---

## Component Deep Dive: Setup SQL

The `setup-sql` binary is a one-shot SQL emitter. It reads a YAML config and renders SQL to stdout. It does **not** connect to any database.

### CLI Interface

```
setup-sql [--log-format <text|json>] <COMMAND>

Commands:
  emit-cockroach-sql --config <PATH> [--format <text|json>]
  emit-postgres-grants --config <PATH> [--format <text|json>]
```

### CockroachDB SQL Emission

For each source database in the config, `emit-cockroach-sql` produces:

1. `SET CLUSTER SETTING kv.rangefeed.enabled = true;` — enables changefeeds
2. `SELECT cluster_logical_timestamp() AS changefeed_cursor;` — captures a timestamp
3. One `CREATE CHANGEFEED FOR TABLE` per mapping — configures a webhook sink pointing at the runner's `/ingest/{mapping_id}` endpoint, with:
   - `cursor = '__CHANGEFEED_CURSOR__'` — placeholder to be replaced with the actual timestamp
   - `initial_scan = 'yes'` — bootstrap existing data
   - `envelope = 'enriched'` — include full before/after row data
   - `enriched_properties = 'source'` — include source database info
   - `resolved = '<interval>'` — periodic resolved timestamp markers
   - `ca_cert = '<base64-encoded-cert>'` — TLS for the webhook connection

### PostgreSQL Grants Emission

For each mapping, `emit-postgres-grants` produces:

1. `GRANT CONNECT, CREATE ON DATABASE "<database>" TO "<role>";`
2. `GRANT USAGE ON SCHEMA "public" TO "<role>";`
3. `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE "<schema>"."<table>" TO "<role>";`

Grants are deduplicated across mappings — if two mappings target the same database and role, only one grant statement is emitted. The tool deliberately does **not** emit `SUPERUSER`, `ALL PRIVILEGES`, or `ALL TABLES IN SCHEMA` grants.

### Output Formats

Both commands support `--format text` (default) and `--format json`:

- **Text**: raw SQL statements separated by empty lines, with comments for context
- **JSON**: `{ "database_name": "sql_string" }` map, one entry per database

---

## Component Deep Dive: Ingest Contract

The `ingest-contract` crate is minimal — it defines a single type `MappingIngestPath` that renders the URL path `/ingest/{mapping_id}`. This is the shared contract between `setup-sql` (which constructs changefeed webhook URLs pointing at this path) and the `runner` (which routes incoming requests at this path pattern).

```rust
let path = MappingIngestPath::new("app-a");
assert_eq!(path.to_string(), "/ingest/app-a");
assert_eq!(path.to_url("https://runner.example.internal:8443"), "https://runner.example.internal:8443/ingest/app-a");
```

The crate has zero dependencies and serves solely to prevent hard-coding the `/ingest/` path convention in multiple places.

---

## Component Deep Dive: Operator Log

The `operator-log` crate provides structured, dual-format logging for operator lifecycle events.

### Log Format

Two formats are supported via `--log-format`:

| Format | Output |
|--------|--------|
| `text` | Only the human-readable `message` field to stderr |
| `json` | A full JSON object per line with `timestamp`, `level`, `service`, `event`, `message`, and arbitrary custom fields |

### Event Model

Events follow a `service.event` naming convention:

| Service | Event | Level | Meaning |
|---------|-------|-------|---------|
| `runner` | `runtime.starting` | info | Async runtime starting |
| `runner` | `runtime.start_failed` | error | Runtime startup failure |
| `runner` | `config.validated` | info | Config validation succeeded |
| `runner` | `command.failed` | error | Top-level command failure |
| `runner` | `webhook.bound` | info | Webhook listener bound |
| `runner` | `webhook.tls_handshake_failed` | error | TLS handshake rejected |
| `runner` | `reconcile.apply_failed` | error | Reconcile pass failure |
| `setup-sql` | `sql.emitted` | info | SQL output produced |
| `setup-sql` | `command.failed` | error | Command failure |

Events are built with a fluent API:

```rust
LogEvent::info("runner", "config.validated", "config validated")
    .with_field("config", &config_path)
    .with_field("mappings", mapping_count)
    .write_to(&mut stderr, log_format)?;
```

All logging goes to **stderr**. Stdout is reserved for command output (SQL payloads, validation results).

---

## Component Deep Dive: Verify Service (MOLT)

The MOLT verify service is a Go binary that provides data verification between source and destination databases.

### CLI Mode: `molt verify`

Direct command-line verification:

```bash
molt verify \
  --source "postgres://root@crdb:26257/demo?sslmode=require" \
  --target "postgres://user@pg:5432/app?sslmode=require" \
  --concurrency 4 \
  --row-batch-size 500 \
  --table-splits 10 \
  --live
```

### Service Mode: `molt verify-service`

HTTP API server that accepts verification jobs:

```bash
molt verify-service run --config verify-service.yml
```

### Verification Pipeline

The verification process has four layers:

1. **Database-level** — queries `pg_class`/`pg_namespace` on both databases, performs a sorted merge-join to find missing, extraneous, and verified tables
2. **Table-level** — for each verified table pair, compares column names, types, nullability, collations, and primary keys. Determines whether the table is safe for row-level verification
3. **Shard-level** — splits verifiable tables into N shards by primary key value ranges (supports integer, float, and UUID PKs)
4. **Row-level** — creates two rate-limited iterators (source/target) that scan rows sorted by PK, compares row-by-row using sorted merge-join logic. Handles type coercion (bool/int, UUID/string, JSON/string, inet/string, timestamp/timestamptz)

### Live Reverification

When `--live` is enabled, mismatches are not immediately reported. Instead, they are queued into a priority queue (min-heap by next retry time) and re-verified using point lookups with exponential backoff. Only mismatches that persist after all retries are reported. This accounts for replication lag where source and destination may temporarily diverge.

### Auto-fix Mode

With `--fixup`, the verify tool automatically applies corrective SQL (`INSERT`, `UPDATE`, `DELETE`) on the target to bring it in sync with the source. This is primarily useful for the initial bootstrap phase.

### HTTP API

The verify service exposes a REST API:

| Method | Path | Description |
|--------|------|-------------|
| `POST /jobs` | Start a verification job (single job at a time) |
| `GET /jobs/{id}` | Poll job status and results |
| `POST /jobs/{id}/stop` | Cancel a running job |
| `POST /tables/raw` | Read raw table rows from source or destination (opt-in) |
| `GET /metrics` | Prometheus metrics |

Job lifecycle: `running` → `succeeded` / `failed` / `stopped`. Only one active job at a time. Only the most recent completed job is retained. State is lost on process restart.

### Error Model

The verify service uses structured operator errors with:

- **Category**: `request_validation`, `job_state`, `source_access`, `mismatch`, `verify_execution`
- **Code**: machine-readable error identifier
- **Message**: human-readable description (URIs sanitized to remove passwords)
- **Details**: optional structured data

---

## Data Flow Diagram

```
┌─────────────────┐     ┌─────────────────────────────────────┐     ┌──────────────────┐
│  CockroachDB     │     │           Runner                     │     │  PostgreSQL       │
│  Source Cluster  │     │                                      │     │  Destination      │
│                  │     │  ┌─────────────────────────────┐     │     │                  │
│  Changefeed ─────┼─────┼─▶│ Webhook Server (HTTPS)       │     │     │                  │
│  (CDC webhook)   │     │  │  POST /ingest/{mapping_id}   │     │     │                  │
│                  │     │  └──────────┬──────────────────┘     │     │                  │
│                  │     │             │                         │     │                  │
│                  │     │  ┌──────────▼──────────────────┐     │     │                  │
│                  │     │  │ Routing & Persistence       │     │     │                  │
│                  │     │  │  Parse JSON ──▶ Route ──▶   │     │     │                  │
│                  │     │  │  Upsert/Delete to shadow    │─────────────▶  _cockroach_migration_tool  │
│                  │     │  │  tables                     │     │     │    .stream_state            │
│                  │     │  └──────────┬──────────────────┘     │     │    .table_sync_state        │
│                  │     │             │                         │     │    .{mapping}__{schema}__{table} │
│                  │     │  ┌──────────▼──────────────────┐     │     │                  │
│                  │     │  │ Resolve Watermark Tracker   │     │     │                  │
│                  │     │  │  Update stream_state         │─────────────▶                  │
│                  │     │  └──────────┬──────────────────┘     │     │                  │
│                  │     │             │                         │     │                  │
│                  │     │  ┌──────────▼──────────────────┐     │     │                  │
│                  │     │  │ Reconcile Loop (periodic)   │     │     │                  │
│                  │     │  │  1. BEGIN                    │     │     │                  │
│                  │     │  │  2. Upsert shadow ──▶ real  │─────────────────────────────────▶│
│                  │     │  │     (FK parent─first)       │     │     │  real tables     │
│                  │     │  │  3. Delete from real where  │─────────────────────────────────▶│
│                  │     │  │     PK not in shadow       │     │     │                  │
│                  │     │  │     (FK child─first)       │     │     │                  │
│                  │     │  │  4. COMMIT                   │     │     │                  │
│                  │     │  └─────────────────────────────┘     │     │                  │
│                  │     │                                      │     │                  │
└─────────────────┘     └─────────────────────────────────────┘     └──────────────────┘
```

```
┌─────────────────┐     ┌─────────────────────────────┐     ┌──────────────────┐
│  CockroachDB     │     │     Verify Service (MOLT)    │     │  PostgreSQL       │
│  Source Cluster  │     │                               │     │  Destination      │
│                  │     │  ┌─────────────────────────┐  │     │                  │
│  pg_class ──────┼─────┼─▶│ DB-level verify         │◀─┼─────┼── pg_class       │
│  pg_attribute   │     │  │ Table-level verify       │  │     │  pg_attribute    │
│  pg_index       │     │  │ Row-level verify         │  │     │  pg_index         │
│  table rows     │     │  │ (sorted merge-join)     │  │     │  table rows      │
│                  │     │  └─────────────────────────┘  │     │                  │
└─────────────────┘     └─────────────────────────────┘     └──────────────────┘
```

---

## Security Considerations

- **No shell in container images**: All three container images use `FROM scratch` — there is no shell, no package manager, and no OS utilities
- **Secrets are not persisted**: Passwords in runner configs are held in memory only; PostgreSQL connection strings are constructed from decomposed fields
- **mTLS supported**: Both the runner webhook and verify service support mutual TLS for client authentication
- **Webhook TLS enforced by default**: CockroachDB changefeeds require HTTPS endpoints, and the runner defaults to `mode: https`
- **URI sanitization**: The verify service sanitizes connection strings in error messages to redact passwords
- **SQL injection protection**: The verify service validates all identifiers against a strict `alphanumeric + underscore` pattern before using them in queries