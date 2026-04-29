# Architecture

How the migration pipeline works internally — sufficient for understanding operator behavior, diagnosing issues, and reasoning about data flow.

## System overview

```
                 ┌─ CockroachDB source ─┐
                 │  changefeeds push     │
                 │  row batches via      │
                 │  webhook-https://     │
                 └──────────┬────────────┘
                            │
                            ▼
  ┌──────────────────────────────────────────────────────┐
  │                    runner container                   │
  │                                                      │
  │  ┌────────────────┐    ┌──────────────────────────┐  │
  │  │ webhook listener│    │     reconcile loop       │  │
  │  │ POST /ingest/:id│───▶│  upsert → delete passes  │  │
  │  └────────────────┘    └──────────┬───────────────┘  │
  │                                   │                   │
  └───────────────────────────────────┼───────────────────┘
                                      │
                                      ▼
  ┌──────────────────────────────────────────────────────┐
  │                PostgreSQL destination                 │
  │                                                      │
  │  ┌─────────────────────────┐  ┌───────────────────┐  │
  │  │ _cockroach_migration_tool│  │  real tables      │  │
  │  │ (helper shadow tables)  │  │  (constrained)    │  │
  │  └─────────────────────────┘  └───────────────────┘  │
  └──────────────────────────────────────────────────────┘
                ▲                       ▲
                │                       │
  ┌─────────────┴───────────────────────┴────────────────┐
  │                verify-service container               │
  │   reads both databases, compares row-by-row           │
  └──────────────────────────────────────────────────────┘
```

## Runner internals

### Webhook listener

The runner exposes `POST /ingest/{mapping_id}` on `webhook.bind_addr`. CockroachDB changefeeds push JSON batches to this endpoint. Each batch contains row events (`c` create, `u` update, `d` delete, `r` refresh) and periodically a `resolved` watermark.

On receiving a batch the runner:

1. Validates the payload structure and `mapping_id`.
2. Opens a PostgreSQL transaction.
3. Applies each row mutation to the corresponding helper shadow table inside `_cockroach_migration_tool`.
4. Updates `latest_received_resolved_watermark` in `stream_state` for resolved messages.
5. Commits and returns `200 OK`.

The runner **never** touches the real constrained tables during webhook handling. This keeps the hot path fast — shadow tables have no foreign keys, secondary indexes, or serving constraints.

### Reconciliation loop

The reconcile loop runs independently on a timer set by `reconcile.interval_secs`. Each tick, for every mapping:

1. **Upsert pass** (parents before children, respecting foreign key order):
   - `INSERT INTO real_table (...) SELECT ... FROM shadow_table ON CONFLICT (...) DO UPDATE ...`
2. **Delete pass** (children before parents, reverse order):
   - `DELETE FROM real_table WHERE NOT EXISTS (SELECT 1 FROM shadow_table WHERE pk matches)`
3. If all tables succeed, advances `latest_reconciled_resolved_watermark` in `stream_state` and updates `last_successful_sync_watermark` per table in `table_sync_state`.

After a successful reconcile pass the real tables and shadow tables are identical for all rows that existed at the reconciled watermark.

### What `reconcile.interval_secs` does operationally

`reconcile.interval_secs` is the number of seconds between two reconciliation passes. Setting it higher gives the destination database more breathing room between bulk upserts and deletes. Setting it lower reduces the lag between webhook ingestion and real-table convergence.

Operationally:

- During bulk initial scans (when the changefeed snapshots millions of rows), longer intervals reduce destination load.
- During steady-state catch-up, shorter intervals keep real tables closer to live.
- The reconcile loop skips a pass if the previous pass is still running — it does not stack concurrent passes.

A value of 30 seconds is a reasonable default for most workloads.

## `_cockroach_migration_tool` helper schema

The runner creates one `_cockroach_migration_tool` schema per destination database. It holds two kinds of objects:

### Tracking tables

| Table | Purpose | Key columns |
|-------|---------|-------------|
| `stream_state` | Per-mapping stream lifecycle | `mapping_id`, `source_database`, `source_job_id`, `starting_cursor`, `latest_received_resolved_watermark`, `latest_reconciled_resolved_watermark`, `stream_status` |
| `table_sync_state` | Per-table reconciliation status | `mapping_id`, `source_table_name`, `helper_table_name`, `last_successful_sync_time`, `last_successful_sync_watermark`, `last_error` |

### Diagnostic queries

Check stream progress (how far CDC has delivered vs how far reconciliation has caught up):

```sql
SELECT mapping_id,
       latest_received_resolved_watermark AS received_up_to,
       latest_reconciled_resolved_watermark AS reconciled_up_to,
       stream_status
FROM _cockroach_migration_tool.stream_state;
```

Check per-table reconciliation status and errors:

```sql
SELECT mapping_id,
       source_table_name,
       last_successful_sync_time,
       last_successful_sync_watermark,
       last_error
FROM _cockroach_migration_tool.table_sync_state;
```

Count shadow rows waiting to be merged into real tables:

```sql
SELECT schemaname, tablename, n_live_tup
FROM pg_stat_user_tables
WHERE schemaname = '_cockroach_migration_tool';
```

### Helper shadow tables

For each mapped source table (e.g. `public.customers`), the runner creates a corresponding shadow table named `{mapping_id}__{schema}__{table}` (e.g. `app-a__public__customers`). Shadow tables mirror the real table's data columns but with:

- **No foreign keys** — avoids constraint ordering problems during upserts
- **No secondary indexes** — keeps writes fast
- **A matching primary key index** — enables efficient upsert and anti-join delete passes

Shadow tables are the durable landing zone for changefeed batches. If the runner crashes between webhook ingestion and reconcile, no data is lost — the shadow table holds every received row and reconciliation resumes from where it left off.

## Verify-service internals

### How table comparison works

When a verify job starts (`POST /jobs`), the verify-service:

1. **Connects to both databases** using the configured `verify.source` and `verify.destination` connection strings.
2. **Discovers all user tables** on each side by querying `pg_class` / `pg_namespace`, excluding system schemas (`pg_catalog`, `information_schema`, `crdb_internal`, `pg_extension`).
3. **Applies filters** from the job request body:
   - `include_schema` / `include_table` — POSIX regexes that tables must match to be verified (default `.*`, matching everything)
   - `exclude_schema` / `exclude_table` — POSIX regexes that exclude matching tables
4. **Compares table lists** across the two databases:
   - Tables in source but not destination → reported as **missing**
   - Tables in destination but not source → reported as **extraneous**
   - Tables in both → **verified** (columns compared, then row data)
5. **Compares column definitions** for each verified table, reporting mismatches.
6. **Splits each table into shards** by primary key range (default 8 shards per table).
7. **Compares row data** within each shard in parallel (default 8 concurrent workers), batching 1000 rows at a time from both sides.
8. **Reports findings** — matching rows, missing rows, extraneous rows, and per-column value mismatches.

### Runner ↔ verify-service separation

The runner and verify-service are completely independent processes that share no runtime state:

- The **runner** writes webhook data into PostgreSQL and reconciles it into real tables. It does not verify correctness.
- The **verify-service** reads both databases and compares them. It does not participate in data movement.

They communicate only through the database: the runner populates destination tables; the verify-service queries them. This separation means you can run verification at any time, with any cadence, without affecting the migration pipeline.

The verify-service compares **source directly against destination real tables** — not against the `_cockroach_migration_tool` shadow tables. So a verify job measures the end-to-end correctness of the full pipeline: changefeed → webhook → shadow table → reconcile → real table.

### Why verify-service uses separate database connections

The verify-service connects to both source and destination using the `postgresql://` URLs in its config. It can connect to any PostgreSQL-compatible database — CockroachDB, standard PostgreSQL, or managed services. It does not depend on the runner's configuration or connection state. This also means the verify-service can compare two databases that are not connected to any runner at all — useful for one-off comparisons outside a migration.

## Failure modes

### Webhook ingestion failures

When the runner cannot process an incoming changefeed batch:

1. **The transaction is rolled back.** No partial data lands in shadow tables. `500 Internal Server Error` is returned to CockroachDB.
2. **CockroachDB retries.** Changefeeds retry failed deliveries with backoff. During retries the stream advances — the changefeed buffers events and will deliver the current state (not a replay of historical events) on reconnect.
3. **No data is lost.** CockroachDB guarantees at-least-once delivery. The shadow tables remain in their last-committed state. When delivery resumes, newer events arrive and are processed normally. The reconcile loop catches up the real tables from whatever is in the shadow tables.

Common causes: destination database unavailable (transient network blip or PostgreSQL restart), constraint violations from malformed payloads, or shadow table DDL failures during schema bootstrapping.

**What to inspect:**
- Runner stderr logs — every webhook error includes the mapping ID and a description.
- `cockroach_migration_tool_runner_` Prometheus metrics at `GET /metrics`.
- CockroachDB changefeed job status: `SHOW CHANGEFEED JOB <job_id>` on the source cluster.

### Reconciliation failures

When a reconcile pass fails for one or more tables within a mapping:

1. **The failed table reports an error** in `_cockroach_migration_tool.table_sync_state.last_error`. This field stores the most recent error message for that table.
2. **`latest_reconciled_resolved_watermark` is not advanced** for the mapping — the reconcile loop only advances the watermark when every table in the mapping succeeds.
3. **The next reconcile pass retries** on the next timer tick. The reconcile loop does not back off; it retries at the configured interval.
4. **Real tables are not modified** for the failed pass. All changes remain staged in shadow tables. Successful tables from the same mapping are also not advanced because the watermark tracks the whole mapping.
5. **The reconcile loop does not block on a running pass.** If a previous reconcile is still in flight when the timer fires, the tick is skipped.

Common causes: foreign key violations (destination data that conflicts with the shadow-table state), missing required destination columns, or destination database becoming unavailable mid-pass.

**What to inspect:**
- `_cockroach_migration_tool.table_sync_state` — per-table `last_error` and `last_successful_sync_time`:
  ```sql
  SELECT mapping_id, source_table_name,
         last_successful_sync_time,
         last_error
  FROM _cockroach_migration_tool.table_sync_state
  WHERE last_error IS NOT NULL;
  ```
- `_cockroach_migration_tool.stream_state` — check if `latest_reconciled_resolved_watermark` has stalled relative to `latest_received_resolved_watermark`:
  ```sql
  SELECT mapping_id,
         latest_received_resolved_watermark AS received_up_to,
         latest_reconciled_resolved_watermark AS reconciled_up_to,
         stream_status
  FROM _cockroach_migration_tool.stream_state;
  ```
- Runner stderr logs — reconciliation errors are logged with the mapping ID and the specific table that failed.
- Shadow table row counts versus real table row counts — if reconciled watermark is stalled but shadow tables contain rows, reconciliation is blocked on something (constraint, missing column, etc.).

### Runner process crash

- **Shadow tables are durable.** All committed webhook batches survive a crash.
- **Reconciliation resumes from the last committed watermark.** On restart the runner bootstraps `_cockroach_migration_tool` (no-op if already present), then begins the reconcile loop.
- **Stream state is in-database.** `stream_status`, watermarks, and per-table sync state persist across restarts.
- **No double processing.** The runner tracks watermarks in the database; it does not reprocess already-reconciled batches.

### Verify-service failures

- **Connection failures** during a verify job produce `source_access` or `destination_access` errors. The job status becomes `failed` with the error category and message in the response body.
- **A running job survives transient connection errors** to one database — the other database's shards continue in parallel. The job fails only when all workers have exhausted their shards or hit unrecoverable errors.
- **Job state is in-memory.** If the verify-service process crashes, all job history is lost. Start a new job after restart.
- **Only one job runs at a time.** `POST /jobs` returns `409 Conflict` if a job is already running. Stop it first with `POST /jobs/{job_id}/stop`.
