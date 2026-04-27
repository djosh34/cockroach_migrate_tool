# Architecture

## Migration Goal: How It Works End-to-End

The tool migrates live data from CockroachDB to PostgreSQL with zero downtime on the source, using CockroachDB's native Change Data Capture (CDC) changefeeds. The goal is to maintain a PostgreSQL copy that stays continuously in sync with the CockroachDB source until cutover.

### High-Level Data Flow

```
┌─────────────────┐    CREATE CHANGEFEED     ┌──────────────────┐
│  CockroachDB    │ ─────────────────────────→│   CockroachDB    │
│  (source)       │   kv.rangefeed emits     │   changefeeds    │
│                 │   row-level changes       │   (per mapping)  │
└─────────────────┘                          └────────┬─────────┘
                                                      │
                              webhook HTTPS POST       │
                              /ingest/{mapping_id}     │
                                                      ▼
┌──────────────────────────────────────────────────────────────────┐
│                          runner                                    │
│                                                                    │
│  ┌─────────────────────┐        ┌──────────────────────────────┐ │
│  │  Webhook Runtime     │        │  Reconcile Runtime            │ │
│  │  (axum HTTP/S)       │───────→│  (per-mapping loop)           │ │
│  │                      │  shadow │                               │ │
│  │  POST /ingest/{id}   │  tables │  upsert → real tables         │ │
│  │  GET /healthz         │        │  delete → real tables         │ │
│  │  GET /metrics         │        │  (FK-aware ordering)          │ │
│  └─────────────────────┘        └──────────────────────────────┘ │
│                                                                    │
└─────────────────────────────┬────────────────────────────────────┘
                              │ PostgreSQL wire protocol (sqlx)
                              ▼
┌──────────────────────────────────────────────────────────────────┐
│                     PostgreSQL (destination)                       │
│                                                                    │
│  ┌─────────────────────────────────────────────┐                  │
│  │  _cockroach_migration_tool                  │                  │
│  │   ├── stream_state           (tracking)     │                  │
│  │   └── table_sync_state       (per-table)    │                  │
│  └─────────────────────────────────────────────┘                  │
│  ┌─────────────────────────────────────────────┐                  │
│  │  Shadow (helper) tables                     │                  │
│  │   ├── app-a__public__customers              │                  │
│  │   ├── app-a__public__orders                 │                  │
│  │   └── app-b__public__invoices               │                  │
│  └─────────────────────────────────────────────┘                  │
│  ┌─────────────────────────────────────────────┐                  │
│  │  Real destination tables                    │                  │
│  │   ├── public.customers                      │                  │
│  │   ├── public.orders                         │                  │
│  │   └── public.invoices                       │                  │
│  └─────────────────────────────────────────────┘                  │
└──────────────────────────────────────────────────────────────────┘
```

The migration strategy has three phases:

### Phase 1: Setup (operator action)

The operator uses **setup-sql** to generate SQL for:
1. Creating changefeeds on CockroachDB that push row changes to the runner via webhooks
2. Granting necessary PostgreSQL permissions to the runtime role

### Phase 2: Continuous Replication (runner)

The **runner** runs continuously:
1. CockroachDB changefeeds emit row-level events (inserts, updates, deletes) + periodic resolved timestamps
2. The runner's webhook runtime receives these events and persists them immediately into **shadow tables** on PostgreSQL
3. On a configurable interval, the **reconcile runtime** copies data from shadow tables into the **real destination tables** and removes rows that were deleted at the source

### Phase 3: Verification (operator action)

The **verify service** (on-demand) compares source and destination data to confirm consistency, useful for pre-cutover validation.

---

## Component: setup-sql

**Language:** Rust
**Image:** `quay.io/<org>/setup-sql:<tag>`
**Binary:** `setup-sql`

The setup-sql tool is a stateless CLI that emits SQL. It does not connect to any database — it only reads config and renders output. The operator must run the output SQL manually against the CockroachDB and PostgreSQL clusters.

### Commands

#### `emit-cockroach-sql`

Reads `cockroach-setup.yml` and outputs SQL for the CockroachDB source cluster.

**What it renders:**

1. `SET CLUSTER SETTING kv.rangefeed.enabled = true;` — Required on CockroachDB for changefeeds
2. `SELECT cluster_logical_timestamp() AS changefeed_cursor;` — Captures the current logical timestamp as a cursor
3. `CREATE CHANGEFEED FOR TABLE <tables> INTO 'webhook-<base_url>/ingest/<id>?...' WITH cursor = ..., initial_scan = 'yes', envelope = 'enriched', enriched_properties = 'source', resolved = <interval>;`

**Changefeed configuration details:**

- `envelope = 'enriched'` — Each event includes the row's before/after state plus operation metadata
- `enriched_properties = 'source'` — Includes the source database name and table name in each event
- `initial_scan = 'yes'` — Emits full table contents as insert operations before starting live streaming
- `resolved` — Periodic boundary markers that guarantee all changes up to a timestamp have been emitted
- `cursor` — The starting point; the operator captures the current timestamp before running the CREATE

**CA certificate encoding:** The `ca_cert_path` file is read, base64-encoded, and percent-encoded into a query parameter on the sink URL. CockroachDB uses this to trust the runner's TLS server certificate during the webhook connection.

#### `emit-postgres-grants`

Reads `postgres-grants.yml` and outputs SQL for the PostgreSQL destination.

**What it renders per destination database:**

```sql
-- PostgreSQL grants SQL
-- Destination database: app_a
-- Helper schema: _cockroach_migration_tool

GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;
GRANT USAGE ON SCHEMA public TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.orders TO migration_user_a;
```

The `CREATE` privilege on the database is needed so the runner can create the `_cockroach_migration_tool` schema and shadow tables at bootstrap.

### Output Formats

- `--format text` (default) — Human-readable SQL with comments
- `--format json` — JSON object mapping database name to SQL string, suitable for programmatic consumption

---

## Component: runner

**Language:** Rust
**Image:** `quay.io/<org>/runner:<tag>`
**Binary:** `runner`

The runner is a long-running daemon. It has three phases at startup, then runs two concurrent subsystems.

### CLI

```
runner run --config <path>
runner validate-config --config <path> [--deep]
```

Global flag: `--log-format` (text or json).

### Startup: Configuration & Planning

1. **Load config** — Parse `runner.yml` using strict deserialization (`deny_unknown_fields`). Validate all fields: non-empty strings, valid socket addresses, TLS file existence, unique mapping IDs, schema-qualified table names.

2. **Build startup plan** (`RunnerStartupPlan`) — Group mappings by destination PostgreSQL database. Each `DestinationGroupPlan` contains all mappings targeting the same PostgreSQL instance.

3. **Bootstrap PostgreSQL** — For each destination group:
   - Connect to PostgreSQL
   - Create the `_cockroach_migration_tool` schema if it doesn't exist
   - Create the `stream_state` and `table_sync_state` tracking tables
   - Inspect the existing PostgreSQL schema (`information_schema.columns`, `pg_catalog` constraint tables) to build a `ValidatedSchema` for each referenced table
   - Create shadow (helper) tables for each mapped source table
   - Seed tracking state rows

4. **Build runtime plan** (`RunnerRuntimePlan`) — Pair each mapping with its `MappingHelperPlan` (helper table metadata + reconcile ordering) and organize the upsert/delete order tables.

### Subsystem 1: Webhook Runtime

The webhook runtime is an `axum` HTTP(S) server that receives CockroachDB changefeed events.

**Endpoints:**

| Method | Path | Description |
|---|---|---|
| `GET` | `/healthz` | Returns `ok` (liveness probe) |
| `GET` | `/metrics` | Prometheus text format metrics |
| `POST` | `/ingest/{mapping_id}` | Changefeed webhook sink |

**TLS modes:**

- `http` — Plain HTTP (for development/testing only)
- `https` — TLS with server certificate (required for CockroachDB webhook sinks)
- `https` + `client_ca_path` — mTLS: the server verifies client certificates against the configured CA

The TLS layer uses `rustls` with the `ring` crypto provider and supports HTTP/2 (h2) and HTTP/1.1 via ALPN negotiation.

**Request processing flow:**

1. Receive `POST /ingest/{mapping_id}` with JSON body
2. Look up the mapping by `mapping_id` → returns 404 if unknown
3. Parse the JSON payload (see below) → returns 400 on parse error
4. Route to either a row batch handler or a resolved timestamp handler
5. Persist to PostgreSQL shadow tables → returns 500 on database error
6. Record metrics (request count by kind + outcome, apply duration)

**Payload format:**

The runner accepts two payload shapes, auto-detected from the JSON structure:

**Row batch** (array of change events):

```json
{
  "length": 2,
  "payload": [
    {
      "source": {
        "database_name": "demo_a",
        "schema_name": "public",
        "table_name": "customers"
      },
      "op": "c",
      "key": {"id": 42},
      "after": {"id": 42, "name": "Acme Corp", "email": "acme@example.com"}
    },
    {
      "source": {
        "database_name": "demo_a",
        "schema_name": "public",
        "table_name": "customers"
      },
      "op": "d",
      "key": {"id": 99},
      "after": null
    }
  ]
}
```

Operation codes:
- `c` (create) → upsert
- `u` (update) → upsert
- `r` (read/initial scan) → upsert
- `d` (delete) → delete

**Resolved timestamp:**

```json
{
  "resolved": "1735000000000000000.0000000000"
}
```

A resolved event is a watermark: CockroachDB guarantees that all row changes with timestamps before this value have been emitted. The runner records the highest (monotonically increasing) resolved watermark per mapping for use by the reconcile runtime.

**Persistence to shadow tables:**

The routing layer validates that:
- The mapping ID exists
- The source database and table in the event match the expected mapping configuration
- Each batch contains events for only a single table (single-table batches enforced)

For **upserts**, the persistence layer:
1. Connects to the destination PostgreSQL
2. Begins a transaction
3. For each row: renders and executes `INSERT INTO shadow_table SELECT * FROM jsonb_populate_record(NULL::shadow_table, $1::jsonb) ON CONFLICT (pk_cols) DO UPDATE SET ...`

For **deletes**, the persistence layer:
1. Requires that the shadow table has primary key columns (errors if not)
2. Renders and executes `DELETE FROM shadow_table AS target USING jsonb_populate_record(NULL::shadow_table, $1::jsonb) AS key_data WHERE pk_cols IS NOT DISTINCT FROM key_data.pk_cols`

This approach leverages PostgreSQL's `jsonb_populate_record` to deserialize the JSON payload directly into typed table columns, avoiding manual column mapping.

### Subsystem 2: Reconcile Runtime

The reconcile runtime periodically copies data from shadow tables into the real destination tables and cleans up deleted rows.

**Concurrency model:** One tokio task spawned per per-(destination_group, mapping) pair. The runtime stops (with an error) if any single task fails — all tasks share the same fate via `tokio::try_join!`.

**Loop structure:**

```
loop forever:
    sleep(interval)
    begin transaction
    for each table in upsert order:
        INSERT INTO real_table (...) SELECT ... FROM shadow_table ON CONFLICT (pk) DO UPDATE SET ...
    for each table in delete order (reverse of upsert order):
        DELETE FROM real_table WHERE NOT EXISTS (SELECT 1 FROM shadow_table WHERE pk_match)
    update tracking state (watermarks, sync times)
    commit transaction
    refresh Prometheus metrics
```

**Upsert SQL generation** (`reconcile_runtime/upsert.rs`):

For each shadow table:
```sql
INSERT INTO public.customers (id, name, email)
SELECT helper.id, helper.name, helper.email
FROM _cockroach_migration_tool.app_a__public__customers AS helper
ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name, email = EXCLUDED.email
```

- Primary key columns form the conflict target
- Non-primary-key, non-generated columns form the UPDATE assignments
- Tables with only primary key columns use `DO NOTHING`
- Generated columns are excluded from both INSERT and SET clauses

**Delete SQL generation** (`reconcile_runtime/delete.rs`):

For each shadow table:
```sql
DELETE FROM public.customers AS target
WHERE NOT EXISTS (
    SELECT 1
    FROM _cockroach_migration_tool.app_a__public__customers AS helper
    WHERE helper.id IS NOT DISTINCT FROM target.id
)
```

- Uses `IS NOT DISTINCT FROM` to handle NULL-safe primary key comparison
- Only deletes rows from the real table that have no corresponding row in the shadow table

**Foreign key ordering** (`helper_plan.rs`):

The runner uses **topological sorting** (Kahn's algorithm) on the foreign key dependency graph to determine correct reconcile ordering:

- **Upsert order** = topological order (parents before children) — ensures FK constraints are satisfied when inserting
- **Delete order** = reverse topological order (children before parents) — ensures FK constraints are satisfied when deleting

If a cycle exists in the FK graph, startup fails with a clear error message listing the tables involved in the cycle.

### Tracking State (`_cockroach_migration_tool` schema)

The runner maintains two tracking tables in the destination PostgreSQL:

#### `stream_state`

| Column | Type | Purpose |
|---|---|---|
| `mapping_id` | TEXT PK | References the config mapping ID |
| `source_database` | TEXT | CockroachDB source database name |
| `source_job_id` | TEXT | CockroachDB changefeed job ID (operator-supplied) |
| `starting_cursor` | TEXT | Cursor used when creating the changefeed |
| `latest_received_resolved_watermark` | TEXT | Most recent resolved timestamp from webhook |
| `latest_reconciled_resolved_watermark` | TEXT | Most recent resolved timestamp that has been reconciled |
| `stream_status` | TEXT | `bootstrap_pending` (initial), can be extended |

#### `table_sync_state`

| Column | Type | Purpose |
|---|---|---|
| `mapping_id` | TEXT | FK to stream_state |
| `source_table_name` | TEXT | Fully-qualified source table name |
| `helper_table_name` | TEXT | Name of the corresponding shadow table |
| `last_successful_sync_time` | TIMESTAMPTZ | When reconciliation last succeeded |
| `last_successful_sync_watermark` | TEXT | Resolved watermark at last success |
| `last_error` | TEXT | Error message from the most recent failure |

**Watermark flow:**

```
CockroachDB emits resolved: "1735..."
       │
       ▼
Webhook receives → persists to stream_state.latest_received_resolved_watermark
                                            │
                    On next reconcile pass  ▼
                  stream_state.latest_reconciled_resolved_watermark
                  table_sync_state.last_successful_sync_watermark
```

Watermarks are updated using monotonic SQL logic:
```sql
UPDATE _cockroach_migration_tool.stream_state
SET latest_received_resolved_watermark = CASE
    WHEN latest_received_resolved_watermark IS NULL
      OR latest_received_resolved_watermark < $2
    THEN $2
    ELSE latest_received_resolved_watermark
END
WHERE mapping_id = $1
```

### Shadow (Helper) Tables

For each mapped source table, the runner creates a shadow table under the `_cockroach_migration_tool` schema. The naming convention is:

```
{mapping_id}__{schema}__{table}
```

Example: `app-a__public__customers`

**Table structure:** Shadow tables mirror the source table's column definitions (name, data type, nullability) but have **no constraints** except a single unique index on the primary key columns. This maximizes write throughput while still supporting upsert conflict detection.

**DDL generated at bootstrap:**

```sql
CREATE TABLE IF NOT EXISTS _cockroach_migration_tool."app-a__public__customers" (
    id INT8 NOT NULL,
    name TEXT,
    email TEXT
);
CREATE UNIQUE INDEX IF NOT EXISTS "app-a__public__customers__pk"
    ON _cockroach_migration_tool."app-a__public__customers" (id);
```

### Metrics

The runner exposes a Prometheus metrics endpoint at `GET /metrics` on the webhook server port.

**Metric families:**

| Metric | Type | Labels | Description |
|---|---|---|---|
| `cockroach_migrate_webhook_requests_total` | Counter | `mapping_id`, `kind` (row_batch/resolved), `outcome` (ok/bad_request/internal_error) | Webhook request counts |
| `cockroach_migrate_webhook_apply_duration_seconds` | Histogram | `mapping_id` | Row batch apply duration |
| `cockroach_migrate_reconcile_apply_attempts_total` | Counter | `mapping_id`, `phase` (upsert/delete) | Reconcile apply attempts |
| `cockroach_migrate_reconcile_apply_duration_seconds` | Histogram | `mapping_id`, `phase` | Per-pass apply duration |
| `cockroach_migrate_table_shadow_row_count` | Gauge | `mapping_id`, `table` | Row count in shadow table |
| `cockroach_migrate_table_real_row_count` | Gauge | `mapping_id`, `table` | Row count in real table |
| `cockroach_migrate_reconcile_errors_total` | Counter | `mapping_id`, `table`, `phase` | Reconcile error count |
| `cockroach_migrate_reconcile_last_success_timestamp_seconds` | Gauge | `mapping_id` | Unix timestamp of last successful reconcile |

### Error Handling

**Transactions:** Both the webhook persistence and reconcile runtime operate in PostgreSQL transactions. If a reconcile pass fails mid-way (e.g., constraint violation), the entire transaction is rolled back — no partial applies.

**Reconcile failure recording:** On failure, a new connection is opened (since the original transaction is rolled back) to record the error in `table_sync_state.last_error`. The error includes the phase (upsert/delete), the table, and the full error detail.

**Structured logging:** All errors are emitted as structured `LogEvent` records with machine-readable fields for mapping ID, database, table, phase, and error detail.

---

## Component: verify (MOLT)

**Language:** Go
**Image:** `quay.io/<org>/verify:<tag>`
**Binary:** `molt`

The verify service is based on CockroachDB's MOLT (Migrate Off Legacy Things) tool. It provides an HTTP API for comparing data between CockroachDB source and PostgreSQL destination databases.

### CLI

```
molt verify --source <conn-str> --destination <conn-str> [--live] [--continuous] [--table-filter <regex>]
molt verify-service validate-config --config <path>
molt verify-service run --config <path> [--log-format text|json]
```

### HTTP API

The verify service exposes a REST API over HTTPS:

| Method | Path | Description |
|---|---|---|
| `POST` | `/jobs` | Start a new verification job |
| `GET` | `/jobs/{id}` | Get job status and results |
| `POST` | `/jobs/{id}/stop` | Stop a running job |
| `POST` | `/tables/raw` | Read raw table data (debugging) |
| `GET` | `/metrics` | Prometheus metrics |

**Job lifecycle:**

1. `POST /jobs` creates a job and starts verification immediately. Only one job runs at a time; submitting a second job rejects with 409 Conflict while another is running.
2. The service retains the last completed job indefinitely (overwritten by the next job).
3. `POST /jobs/{id}/stop` cancels a running job.

**Job request body:**

```json
{
  "table_filter": ".*",
  "schema_filter": "public",
  "live": false,
  "continuous": false
}
```

- `live` — Re-verify rows that initially mismatch to rule out transient inconsistencies
- `continuous` — Run verification in a loop until stopped
- `table_filter` / `schema_filter` — POSIX regex patterns to include tables/schemas

**Job result:** The response includes summary counts (total tables, rows verified, mismatches), per-table detail, and a list of specific row-level mismatches with both source and destination values.

### Verification Engine

The verify module uses two approaches to compare data:

1. **Table-level verification** — Runs a hash-based comparison by splitting tables into shards, computing checksums on each side, and comparing. Fast for tables of any size.

2. **Row-level verification** — For tables flagged as mismatched, iterates through rows using either scan-based or point-lookup-based strategies, comparing each row. Useful for detailed mismatch reporting.

### Database Abstraction

The `dbconn` package provides a common interface (`Conn`) with implementations for:
- `PGConn` — PostgreSQL (pgx driver)
- `CRDBConn` — CockroachDB (pgx driver with CockroachDB-specific features)
- `FakeConn` — For testing

### Data Type Conversion

The `pgconv` package handles converting CockroachDB-specific data types to their PostgreSQL equivalents during comparison, using the `typeconv` utilities for type mapping and description.

---

## Shared Libraries (Rust)

### ingest-contract

A trivial crate defining `MappingIngestPath` — a URL path struct that renders `/ingest/{mapping_id}`. This is the contract between setup-sql (which constructs changefeed sink URLs) and the runner (which routes webhook requests by mapping ID).

### operator-log

Structured logging with two output formats:

```rust
// Text mode
LogEvent::info("runner", "reconcile.completed", "reconcile pass completed")
    .with_field("mapping_id", "app-a")
    .write_to(&mut std::io::stderr());

// JSON mode — same API, outputs JSON lines
```

Log events include timestamps (RFC 3339), component name, event type, message, and arbitrary key-value fields.

---

## Image Pipeline

### Docker Build Strategy

All images use multi-stage builds:

1. **Chef stage** — Install `cargo-chef`, prepare recipe for dependency caching
2. **Planner stage** — Compute dependency tree
3. **Builder stage** — Compile the binary with `cargo chef cook` (cached deps) then `cargo build` (source code)
4. **Runtime stage** — `scratch` base with only the statically-linked binary

This produces minimal images (< 20 MB compressed) with no shell, no package manager, and no OS utilities.

### Multi-Architecture

Images are built for `linux/amd64` and `linux/arm64`. The `Dockerfile` uses `TARGETARCH` to select the correct Rust target triple (`x86_64-unknown-linux-musl` or `aarch64-unknown-linux-musl`).

Multi-arch manifests are published via `docker buildx` with parallel platform builds. Manifest lists are pushed to both Quay.io (primary) and GHCR (mirror).

### CI/CD Pipeline (`.github/workflows/publish-images.yml`)

1. **validate-fast** — `cargo clippy -D warnings` + `cargo test --workspace`
2. **validate-long** — `cargo test --workspace -- --ignored --test-threads=1`
3. **publish-image** (per platform, per image) — Build, tag, push single-platform image
4. **quay-security-gate** — Vulnerability scan on pushed images
5. **publish-manifest** — Create and push multi-arch manifest lists to Quay and GHCR

---

## TLS Configuration Summary

The project uses TLS at multiple boundaries:

| Connection | TLS Responsibility | Configuration |
|---|---|---|
| CockroachDB → runner webhook | CockroachDB as HTTPS client | Runner's cert signed by CA in `ca_cert_path` |
| Runner → PostgreSQL destination | sqlx (rustls backend) | `mappings[].destination.tls` with mode (require/verify-ca/verify-full) and cert paths |
| Verify → Source CockroachDB | pgx driver | TLS in connection string (sslmode, sslrootcert, sslcert, sslkey) |
| Verify → Destination PostgreSQL | pgx driver | TLS in connection string |
| Client → Verify API | Verify as HTTPS server | Verify's server cert + optional mTLS via `client_ca_path` |
