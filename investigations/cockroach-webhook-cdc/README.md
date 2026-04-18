# CockroachDB Webhook CDC Investigation

## Goal

Answer these questions with a repeatable Docker-based setup:

- If the database already contains data, what happens when a CockroachDB webhook changefeed is created afterward?
- What does the webhook request body actually look like?
- Can existing rows be transferred too, not only new changes?
- What does this imply for building a CockroachDB -> PostgreSQL CDC pipeline?

## How To Rerun

From this directory:

```bash
./scripts/run.sh
```

What it does:

1. Generates a local CA plus HTTPS server certificate for the webhook receiver.
2. Starts CockroachDB `v26.1.2` and an HTTPS request-capture receiver with Docker Compose.
3. Creates a multi-table schema with foreign keys and indexes.
4. Seeds non-trivial data:
   - `customers`: 24 rows
   - `products`: 18 rows
   - `orders`: 72 rows
   - `order_items`: 216 rows
5. Creates:
   - a live enriched webhook changefeed with an initial scan
   - a snapshot-only webhook changefeed
   - a source-metadata probe changefeed
6. Applies post-changefeed mutations and captures all webhook requests under `output/requests/`.

## Dataset

Schema:

- `customers`
- `products`
- `orders` -> FK to `customers`
- `order_items` -> FK to `orders` and `products`

Indexes:

- `customers_region_status_idx`
- `products_category_active_idx`
- `orders_customer_created_idx`
- `orders_status_idx`
- `order_items_product_idx`

Seeded counts are in `output/sql/01_row_counts.txt`.

## What Actually Happened

### 1. Existing rows were transferred after CDC was enabled

Yes.

The live changefeed was created **after** the tables were already populated. CockroachDB immediately emitted the existing current state as webhook events.

Observed in `output/summary.json`:

- `/enriched-live`: `334` create-like row events, `4` updates, `1` delete, `8` resolved messages
- `/snapshot-only`: `333` create-like row events and then the job finished

Why the counts differ:

- Initial seed state = `330` rows total
- Live mutation transaction inserted `4` rows and deleted `1`, so the later snapshot-only feed saw `333` current rows
- The live feed also saw later updates and deletes because it stayed running

### 2. Existing rows looked like inserts

This is important.

For the live feed with `envelope='enriched'` and `diff`, the initial scan emitted existing rows as:

- `op: "c"`
- `before: null`

That means pre-existing rows are represented like creates in the payload. There is no special `"this came from the initial backfill"` marker in the row payload itself.

Example: `output/requests/0002-enriched-live.json`

```json
{
  "payload": [
    {
      "after": {
        "created_at": "2026-01-05T12:00:00Z",
        "customer_id": 1,
        "id": 1,
        "order_number": "ORD-00001",
        "paid_at": null,
        "shipping_country": "DE",
        "status": "pending",
        "total_cents": 13475
      },
      "before": null,
      "key": { "id": 1 },
      "op": "c",
      "ts_ns": 1776523202550657859
    }
  ],
  "length": 25
}
```

### 3. Webhook request shapes

There were **three distinct request shapes** in the captured artifacts.

#### A. Row batch request

Observed in `output/requests/0016-enriched-live.json`.

```json
{
  "payload": [
    {
      "after": { "...": "..." },
      "before": { "...": "..." },
      "key": { "...": "..." },
      "op": "u",
      "ts_ns": 1776523212739289741
    }
  ],
  "length": 7
}
```

Notes:

- Requests are batched.
- `length` is the number of row events in the batch.
- Deletes came as `after: null` plus a populated `before`.
- For this enriched feed, the payload contained `after`, `before`, `key`, `op`, and top-level event `ts_ns`.

#### B. Resolved watermark request

Observed in `output/requests/0015-enriched-live.json`.

```json
{
  "resolved": "1776523202550664778.0000000000"
}
```

Notes:

- Resolved messages are **not** wrapped in `"payload"`.
- A consumer must branch on body shape:
  - row batch: `{"payload":[...],"length":N}`
  - progress watermark: `{"resolved":"..."}`

#### C. Enriched source-metadata row request

Observed in `output/requests/0036-enriched-source.json`.

```json
{
  "payload": [
    {
      "after": {
        "created_at": "2026-01-02T08:00:00Z",
        "email": "customer-4@example.com",
        "id": 4,
        "region": "priority-east",
        "status": "active"
      },
      "key": { "id": 4 },
      "op": "u",
      "source": {
        "database_name": "demo_cdc",
        "schema_name": "public",
        "table_name": "customers",
        "job_id": "1168024664034050049",
        "mvcc_timestamp": "1776523244559738416.0000000000",
        "ts_hlc": "1776523244559738416.0000000000",
        "ts_ns": 1776523244559738416
      },
      "ts_ns": 1776523244564243816
    }
  ],
  "length": 1
}
```

Notes:

- `source.table_name` is the cleanest observed way to identify the source table in a multi-table feed.
- `source.ts_hlc` / `source.ts_ns` are much more useful for ordering than the top-level `ts_ns`.
- The top-level `ts_ns` looked like changefeed processing time, not commit time.

### 4. Snapshot-only works, but with notable option restrictions

Yes, previous data can be transferred using CDC only, via `initial_scan='only'`.

Observed result:

- The snapshot-only job status became `succeeded`.
- It emitted `333` current rows and then stopped.
- It did **not** receive later mutations.

Observed in `output/sql/02_show_changefeed_jobs.txt` and `output/summary.json`.

However, on CockroachDB `v26.1.2`, the following combinations were rejected for `initial_scan='only'`:

- `initial_scan='only'` + `diff`
- `initial_scan='only'` + `mvcc_timestamp`
- `initial_scan='only'` + `updated`

Those all failed with SQLSTATE `22023`.

Practical meaning:

- Snapshot-only is good for a one-time export of current state.
- But the metadata options are much more limited than for a live changefeed.

### 5. Headers observed

Observed request headers were minimal:

- `Content-Type: application/json`
- `User-Agent: Go-http-client/1.1`
- `Host: host.docker.internal:8443`
- `Accept-Encoding: gzip`

No auth header was used in this investigation.

## Findings That Matter For CockroachDB -> PostgreSQL

### Feasibility

This is feasible, but not "just parse JSON and apply it".

The moment you want a robust pipeline, you need at least:

- an HTTPS webhook receiver
- checkpoint storage
- idempotent apply logic
- primary-key based UPSERT/DELETE behavior
- handling for resolved watermarks
- handling for batches
- handling for row ordering and FK dependency problems
- a strategy for initial load versus live streaming

### Minimum viable design

The safest design from these findings is:

1. Use a dedicated initial load step for PostgreSQL.
2. Start a live changefeed with `envelope='enriched', enriched_properties='source'`.
3. Use resolved messages as checkpoint boundaries.
4. Apply rows idempotently in PostgreSQL by primary key.
5. Use the `source` block for:
   - database/schema/table routing
   - commit ordering
   - checkpoint bookkeeping

### Why not rely on the live feed's initial scan alone?

Because the initial scan makes old rows look like create events:

- `op = "c"`
- `before = null`

That is usable if you deliberately want "current state as inserts", but it is not a true historical replay.

Also, in the live enriched feed without `enriched_properties='source'`, the accepted `updated` and `mvcc_timestamp` options did **not** appear in the webhook body we captured. The `source` enrichment was what finally exposed table and commit metadata cleanly.

### Foreign keys and ordering

This is one of the harder parts.

What I observed:

- Batches are not guaranteed to be one row or one table.
- The live stream included multiple tables.
- A single SQL transaction produced multiple row events in one batch.
- There was no explicit transaction ID in the default enriched payload.

Implication:

- If PostgreSQL applies rows immediately and strictly enforces FKs, out-of-order arrival can hurt you.
- You probably want either:
  - a staging area plus ordered apply, or
  - a replicator that can buffer, collapse, and replay in a dependency-aware way

This is exactly the kind of complexity an existing replicator already solves.

### Existing tool worth studying

Cockroach Labs already documents **MOLT Replicator** for CockroachDB failback over webhook changefeeds. The docs describe it as a webhook sink consumer that buffers mutations, stores checkpoints, and applies time-ordered batches while respecting foreign-key and table dependencies.

That is a strong signal that a production-grade PostgreSQL target replicator is a medium-to-hard project, not a weekend parser.

## Surprise Factor

These were the most useful surprises:

- Existing rows were emitted immediately after enabling the live feed, which is good.
- Existing rows looked exactly like creates, which is less good if you expected true historical semantics.
- Resolved messages were a completely different top-level JSON shape, not another `"payload"` entry.
- `initial_scan='only'` was much more restrictive than expected in this version:
  - no `diff`
  - no `updated`
  - no `mvcc_timestamp`
- `envelope='enriched'` without `enriched_properties='source'` was not enough for a serious multi-table consumer:
  - no table name in the payload
  - no visible `updated`
  - no visible `mvcc_timestamp`
- `enriched_properties='source'` was the first shape that looked genuinely usable for a cross-table replicator.
- The local CockroachDB container also required `SET CLUSTER SETTING kv.rangefeed.enabled = true;` before changefeeds worked in this setup.

## Bottom Line

Short answer:

- Yes, Cockroach webhook CDC can send previously existing rows.
- Yes, it can also do a snapshot-only export of current state.
- The raw webhook body is straightforward JSON, but a serious consumer must handle both row batches and resolved watermark requests.
- For CockroachDB -> PostgreSQL, this is practical but not trivial. Use source-enriched envelopes, checkpoint on resolved timestamps, apply idempotently, and plan for FK/order handling from day one.

## Important Artifacts

- Raw requests: `output/requests/`
- Summary: `output/summary.json`
- Seed row counts: `output/sql/01_row_counts.txt`
- Changefeed job status: `output/sql/02_show_changefeed_jobs.txt`
- Receiver logs: `output/sql/03_receiver_logs.txt`

## Official References

- CockroachDB `CREATE CHANGEFEED`: https://www.cockroachlabs.com/docs/stable/create-changefeed
- CockroachDB changefeed sinks / webhook sink: https://www.cockroachlabs.com/docs/stable/changefeed-sinks
- CockroachDB message envelopes: https://www.cockroachlabs.com/docs/v26.1/changefeed-message-envelopes
- CockroachDB MOLT Replicator: https://www.cockroachlabs.com/docs/molt/molt-replicator
