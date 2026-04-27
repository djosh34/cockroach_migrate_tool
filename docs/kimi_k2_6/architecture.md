# Architecture

This document explains how the migration tool works from the top down: first the overall goal and data flow, then the technical details of each component.

## Migration goal and high-level flow

The objective is to move data from a **CockroachDB** source cluster into a **PostgreSQL** destination with minimal operational complexity. Rather than batch exports or logical replication, the tool uses CockroachDB's built-in **changefeed** capability to stream every insert, update, and delete as a webhook payload. A long-running `runner` process receives those payloads, buffers them in temporary **shadow tables**, and periodically **reconciles** the buffered state into the real destination tables.

This design gives you three important properties:

1. **Near-real-time** — webhooks arrive within seconds of a source transaction committing.
2. **Resumable** — resolved timestamps and tracking tables let you observe progress and resume after restarts.
3. **Transactional safety** — reconciliation happens inside a PostgreSQL transaction, so the destination is always consistent even if a pass fails.

### Data flow overview

```
┌─────────────────┐     changefeed (webhook)      ┌─────────────────┐
│  CockroachDB    │ ─────────────────────────────>│     runner      │
│  source cluster │    enriched envelope, HTTPS   │  (webhook port) │
└─────────────────┘                               └────────┬────────┘
                                                           │
                              ┌────────────────────────────┘
                              │  INSERT / UPDATE / DELETE
                              ▼
                     ┌─────────────────┐
                     │  Shadow tables  │   _cockroach_migration_tool schema
                     │  (PostgreSQL)   │
                     └────────┬────────┘
                              │
                              │ reconcile pass (interval_secs)
                              │  1. upsert shadow rows into real tables
                              │  2. delete real rows not in shadow
                              ▼
                     ┌─────────────────┐
                     │  Real tables    │   your application schema
                     │  (PostgreSQL)   │
                     └─────────────────┘
```

## Workspace layout

The project is a Cargo workspace with four crates:

| Crate | Role | Type |
|-------|------|------|
| `runner` | Long-running service that receives webhooks and reconciles | Binary |
| `setup-sql` | One-shot CLI that emits SQL for changefeeds and grants | Binary |
| `ingest-contract` | Shared URL-path convention (`/ingest/{mapping_id}`) | Library |
| `operator-log` | Structured JSON/text log events with typed fields | Library |

Both binaries use `clap` for CLI parsing, `serde_yaml` for configuration, `sqlx` for PostgreSQL connectivity, and `tokio` for async runtime. The runner additionally uses `axum` for the HTTP server and `rustls` for TLS.

---

## Component: `runner`

The `runner` is the heart of the migration. It has two concurrent runtimes that share a single `RunnerRuntimePlan`:

1. **Webhook runtime** — an HTTP(S) server that ingests changefeed payloads.
2. **Reconcile runtime** — a background worker per mapping that flushes shadow tables into real tables.

### Startup sequence

When `runner run --config <file>` starts, the following happens:

1. **Config loading** — `LoadedRunnerConfig::load` reads and validates the YAML file. It checks socket addresses, TLS material paths, destination URL syntax, and mapping uniqueness.
2. **Startup plan** — `RunnerStartupPlan::from_config` groups mappings by destination database and verifies that no two mappings claim the same destination table.
3. **PostgreSQL bootstrap** — `postgres_bootstrap::bootstrap_postgres` connects to each destination group once and:
   - Creates the `_cockroach_migration_tool` schema if it does not exist.
   - Creates `stream_state` and `table_sync_state` tracking tables.
   - Inspects the live catalog (`pg_attribute`, `pg_constraint`, `pg_class`, `pg_namespace`) to discover columns, primary keys, and foreign keys for every mapped table.
   - Builds a `MappingHelperPlan` that defines shadow table DDL and reconcile ordering.
   - Creates shadow tables and their primary-key unique indexes.
   - Seeds tracking rows so the reconcile loop knows which tables belong to which mapping.
4. **Runtime plan** — `RunnerRuntimePlan::from_startup_plan` combines the startup plan with the helper plans into an immutable, shareable plan held in an `Arc`.
5. **Concurrency** — `tokio::try_join!` launches the webhook runtime and the reconcile runtime. If either fails, the whole process exits.

### Webhook runtime

The webhook runtime binds a TCP listener (plain HTTP or HTTPS with optional mTLS) and serves three routes:

| Route | Method | Purpose |
|-------|--------|---------|
| `/healthz` | `GET` | Kubernetes-style health probe. Returns `ok`. |
| `/metrics` | `GET` | Prometheus-compatible text metrics. |
| `/ingest/{mapping_id}` | `POST` | Receives changefeed payloads for a specific mapping. |

#### Payload parsing

CockroachDB enriched-webhook changefeeds send two shapes:

- **Row batch** — contains `payload`, `length`, and an array of row events. Each event has:
  - `source` with `database_name`, `schema_name`, `table_name`
  - `op` — `c` (create), `u` (update), `r` (read/initial scan), or `d` (delete)
  - `key` — primary-key columns as a JSON object
  - `after` — full row values for upserts (omitted for deletes)

- **Resolved** — contains `resolved` with a CockroachDB HLC timestamp watermark. This tells the runner that all changes up to this time have been sent.

The parser (`webhook_runtime/payload.rs`) validates JSON structure, rejects mixed tables in a single batch, and maps `c|u|r` to `RowOperation::Upsert` and `d` to `RowOperation::Delete`.

#### Persistence

Once a row batch is parsed and routed, it is written to the destination PostgreSQL inside a transaction (`webhook_runtime/persistence.rs`):

- **Upserts** use `jsonb_populate_record` to expand the JSON row into the shadow table, then `ON CONFLICT (primary_key_columns) DO UPDATE`.
- **Deletes** use `jsonb_populate_record` to expand the key, then `DELETE ... USING ... WHERE ... IS NOT DISTINCT FROM ...`.

If the shadow table has no primary key, upserts append rows and deletes are rejected. This is consistent with the reconcile logic, which also requires primary keys for conflict resolution.

#### TLS

If the config specifies `mode: https`, the runtime loads a `rustls` `ServerConfig` from PEM files. An optional `client_ca_path` enables mTLS via `WebPkiClientVerifier`. ALPN is configured for both `h2` and `http/1.1`.

### Reconcile runtime

The reconcile runtime spawns one async worker loop per mapping. Each loop wakes up every `interval_secs` and performs a **reconcile pass**.

#### Reconcile pass

A pass is a single database transaction that does two phases in order:

1. **Upsert phase** — for each table in topological parent-first order, insert or update shadow rows into the real table.
2. **Delete phase** — for each table in reverse topological order, delete real rows that no longer exist in the shadow table.

Topological ordering is computed from foreign-key metadata discovered during bootstrap. Parent tables are reconciled before child tables during upserts, and child tables are reconciled before parent tables during deletes. This prevents foreign-key violations.

If any statement in the pass fails:

- The transaction is rolled back.
- The failure (mapping, table, phase, error message) is written to `table_sync_state.last_error`.
- An operator log event is emitted.
- Metrics are updated.

If the pass succeeds:

- The transaction is committed.
- `stream_state.latest_reconciled_resolved_watermark` is advanced to the latest received resolved watermark.
- `table_sync_state` rows are updated with `last_successful_sync_time` and `last_successful_sync_watermark`.
- Table-row metrics are refreshed from `pg_class`.

#### Why shadow tables instead of direct application?

Writing changefeed events directly to the real tables would create race conditions: a batch of upserts and deletes from different tables could arrive interleaved, and foreign-key constraints could be violated. By landing everything in shadow tables first, the reconcile pass can:

- Reorder work by dependency graph.
- Execute inside a single atomic transaction.
- Retry safely after a failure because the shadow tables still contain the full desired state.

### Metrics

The runner exposes Prometheus-style metrics on `/metrics`. Key families include:

| Metric | Type | Labels | Meaning |
|--------|------|--------|---------|
| `cockroach_migration_tool_webhook_requests_total` | counter | `destination_database`, `kind`, `outcome` | Total webhook requests received. |
| `cockroach_migration_tool_webhook_apply_duration_seconds_total` | counter | `destination_database`, `destination_table` | Total time spent applying webhooks to shadow tables. |
| `cockroach_migration_tool_reconcile_apply_duration_seconds_total` | counter | `destination_database`, `destination_table`, `phase` | Total time spent in reconcile upsert/delete. |
| `cockroach_migration_tool_reconcile_apply_attempts_total` | counter | `destination_database`, `destination_table`, `phase` | Number of reconcile statements executed. |
| `cockroach_migration_tool_table_rows` | gauge | `destination_database`, `destination_table`, `layer` | Row count in shadow vs real tables. |
| `cockroach_migration_tool_table_reconcile_error` | gauge | `destination_database`, `destination_table` | `1` if the table’s last reconcile failed. |
| `cockroach_migration_tool_reconcile_last_success_unixtime_seconds` | gauge | `destination_database`, `destination_table` | Unix timestamp of the last successful reconcile. |
| `cockroach_migration_tool_apply_failures_total` | counter | `destination_database`, `destination_table`, `stage` | Total apply failures across webhook and reconcile stages. |

Metrics are held in a `Mutex<RunnerMetricsState>` and rendered on demand.

### Tracking state schema

The `_cockroach_migration_tool` schema contains two tables:

**`stream_state`** (one row per mapping)

| Column | Purpose |
|--------|---------|
| `mapping_id` | Primary key. Links to the runner config mapping. |
| `source_database` | CockroachDB database name. |
| `source_job_id` | Reserved for future changefeed job tracking. |
| `starting_cursor` | Reserved for resuming from a specific HLC cursor. |
| `latest_received_resolved_watermark` | Highest resolved timestamp received from the webhook. |
| `latest_reconciled_resolved_watermark` | Highest resolved timestamp successfully reconciled. |
| `stream_status` | `bootstrap_pending` or future statuses. |

**`table_sync_state`** (one row per mapping + table)

| Column | Purpose |
|--------|---------|
| `mapping_id` | Part of primary key. |
| `source_table_name` | Part of primary key. |
| `helper_table_name` | Name of the shadow table. |
| `last_successful_sync_time` | `NOW()` when the last reconcile pass succeeded. |
| `last_successful_sync_watermark` | Resolved watermark at the time of success. |
| `last_error` | Human-readable error from the last failed pass. |

---

## Component: `setup-sql`

`setup-sql` is a sidecar CLI that turns YAML configuration into executable SQL. It never touches the network itself; it only renders text.

### `emit-cockroach-sql`

Reads `cockroach-setup.yml` and produces:

1. `SET CLUSTER SETTING kv.rangefeed.enabled = true;`
2. `SELECT cluster_logical_timestamp() AS changefeed_cursor;` — you capture the output and substitute it into the next statement.
3. One `CREATE CHANGEFEED` statement per database grouping. The sink URL is constructed as:
   ```
   webhook-https://<base_url>/ingest/<mapping_id>?ca_cert=<url-encoded-base64-ca>
   ```
   The `ingest-contract` crate provides the `/ingest/{mapping_id}` path convention so that setup-sql and runner stay aligned.

The emitted SQL uses:
- `cursor = '<HLC>'` — ensures the changefeed starts at the exact moment you captured.
- `initial_scan = 'yes'` — backfills existing data.
- `envelope = 'enriched'` with `enriched_properties = 'source'` — includes schema, table, and database metadata in every row event.
- `resolved = '<duration>'` — periodic resolved watermarks so the runner knows when it has seen all changes up to a point in time.

### `emit-postgres-grants`

Reads `postgres-grants.yml` and emits `GRANT` statements per destination database:

- `GRANT CONNECT, CREATE ON DATABASE <db> TO <role>;`
- `GRANT USAGE ON SCHEMA public TO <role>;`
- `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE <schema>.<table> TO <role>;`

This is a convenience helper. You can always manage permissions manually or through your infrastructure-as-usual tool.

---

## Component: `ingest-contract`

A tiny shared library that defines the webhook ingest path convention:

```rust
MappingIngestPath::new(mapping_id).to_url(base_url)
// → "https://runner.example.internal:8443/ingest/demo"
```

Both `setup-sql` and `runner` depend on this crate, so the URL contract is guaranteed to match.

---

## Component: `operator-log`

Provides structured logging with two formats:

- **Text** — human-readable messages to stderr.
- **JSON** — machine-parseable objects with `timestamp`, `level`, `service`, `event`, `message`, and arbitrary typed fields.

Both binaries accept `--log-format json` globally. In JSON mode, successful commands may emit an event object to stderr in addition to the primary stdout payload.

---

## Data integrity and failure modes

| Scenario | Behavior |
|----------|----------|
| Runner restarts | Shadow tables and tracking state persist in PostgreSQL. On restart, bootstrap reconnects, validates schema compatibility, and resumes where it left off. |
| Changefeed lag | Shadow tables grow. Reconcile passes continue to flush whatever is present. There is no back-pressure mechanism in the runner; CockroachDB handles retry and buffering. |
| Reconcile failure | The transaction rolls back. The error is recorded in `table_sync_state.last_error`. The next interval will retry. |
| Schema drift | If the destination schema changes (columns added, primary key altered), the runner must be restarted so bootstrap re-inspects the catalog and recreates shadow tables. |
| Duplicate webhooks | Upserts are idempotent (`ON CONFLICT DO UPDATE`). Duplicate resolved watermarks are ignored by the `CASE WHEN ... <` logic in tracking state. |
| Missing primary key | Tables without a primary key can receive inserts into the shadow table, but deletes and reconciles will fail. The error is surfaced in logs and metrics. |

## Security model

- **Webhook listener** is always HTTPS in production (CockroachDB requires it). TLS certificates are mounted as files; no secrets live in environment variables.
- **PostgreSQL destinations** support `require`, `verify-ca`, and `verify-full` TLS modes, plus optional client certificates.
- **mTLS** on the webhook is supported via `client_ca_path`, useful if a reverse proxy performs client authentication.
- **No secrets in configs** — while the runner config contains `password`, the intended deployment pattern is to inject it via a secrets manager or templating tool before the container starts.
