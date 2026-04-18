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
./scripts/run-molt-verify.sh
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

The second script:

1. Starts Dockerized CockroachDB and PostgreSQL.
2. Loads an equivalent relational dataset into both.
3. Runs `molt verify` once against matching data.
4. Introduces a deliberate Cockroach-only row mismatch.
5. Runs `molt verify` again and captures the resulting warnings and summaries.

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

## MOLT Verify

### What it is

Current official position:

- `MOLT Verify` is a migration validation tool that compares source database tables, column definitions, and row values against CockroachDB.
- The docs currently mark it as **preview**.
- Supported sources in the docs are PostgreSQL 12-16, MySQL 5.7/8.0+, and Oracle 19c/21c.

Useful current capabilities from the docs and release notes:

- `--table-filter` and `--schema-filter` for targeted verification.
- `--row-batch-size` to tune the default 20,000-row batch size.
- `--live` for re-checking rows when data may be changing during verification.
- `--continuous` for looping verification.
- selective data verification via filter predicates was added in the February 26, 2026 release notes.

Current limitations documented by Cockroach Labs:

- schema changes during verification can cause errors
- collation differences on primary-key string columns can fail validation
- geospatial types are not yet comparable
- transformed-data verification is limited to table and schema renames

### What public signal exists

Public sentiment is **thin**.

What I found:

- a substantial amount of official material from Cockroach Labs:
  - product docs
  - release notes
  - an official blog post
  - migration workflow docs
- a Go package page describing `verify`, including `--live` and `--continuous`
- a CockroachDB subreddit demo post showing someone using it in a migration walkthrough

What I did **not** find:

- much independent operator discussion
- many issue threads specifically about `molt verify`
- broad third-party reviews comparing it against other verification tooling

So the honest read is:

- the tool looks actively maintained
- the public ecosystem around it still looks small
- most visible enthusiasm is from Cockroach Labs itself rather than a wide community of operators publishing experiences

### What I actually ran

Repeatable script:

```bash
./scripts/run-molt-verify.sh
```

Artifacts:

- version: `output/molt-verify/version.txt`
- baseline run: `output/molt-verify/baseline.log`
- mismatch run: `output/molt-verify/mismatch.log`
- summarized results: `output/molt-verify/summary.json`

Database setup used for the test:

- PostgreSQL 16 source in Docker
- CockroachDB v26.1.2 target in Docker
- equivalent schema on both sides:
  - `customers`
  - `products`
  - `orders`
  - `order_items`
- same deterministic seed data on both sides:
  - `customers`: 24
  - `products`: 18
  - `orders`: 72
  - `order_items`: 216

Then I ran two cases:

1. Baseline: exact same data on PostgreSQL and CockroachDB.
2. Mismatch: changed `customers.id = 2` only on CockroachDB.

### MOLT Verify results

#### Baseline

It matched cleanly.

Observed in `output/molt-verify/summary.json`:

- `customers`: 24 / 24 matched
- `products`: 18 / 18 matched
- `orders`: 72 / 72 matched
- `order_items`: 216 / 216 matched
- exit code: `0`
- completion message: `verification complete`

This means `molt verify` worked correctly for this equivalent Postgres -> Cockroach dataset.

#### Mismatch case

It detected the row difference correctly.

Observed warning in `output/molt-verify/mismatch.log`:

```json
{
  "type": "data",
  "table_schema": "public",
  "table_name": "customers",
  "source_values": { "status": "paused" },
  "target_values": { "status": "crdb-only-mismatch" },
  "primary_key": ["2"],
  "message": "mismatching row value"
}
```

Observed summary:

- `customers`: `num_mismatch = 1`, `num_success = 23`, `num_truth_rows = 24`
- all other tables remained fully matched

So on the core question:

- yes, the tool detected a real mismatch
- yes, the matching case genuinely matched

### Friction I hit

This part is important.

#### 1. The documented/latest version story was messy

- Cockroach Labs release notes list `molt 1.3.6` as available on February 26, 2026.
- The Docker tag `cockroachdb/molt:1.3.6` was **not** available when I tried to pull it.
- `cockroachdb/molt:latest` did work and reported `molt version v1.3.7`.

That is a real reproducibility friction point. In the script I pinned the working image digest instead of relying on a missing semver tag.

#### 2. MOLT refused insecure TLS by default

Because both databases in this investigation are local/dev and use insecure connections, `molt verify` initially refused to run until I added:

```bash
--allow-tls-mode-disable
```

That is good from a safety perspective, but it is still setup friction you have to know about.

#### 3. The Linux Docker connection advice did not work for this setup

The docs say that for local Docker usage on Linux you can use `172.17.0.1`.

In this investigation, that was not enough because the CockroachDB container was running in insecure single-node mode bound in a way that was reachable from inside its own container namespace but not cleanly from another container over the published host port.

What worked reliably was:

- run the MOLT container in Cockroach’s network namespace
- connect to Cockroach on `127.0.0.1:26257`
- connect to Postgres by service name `postgres:5432`

This is not terrible, but it is definitely more fiddly than the happy-path docs suggest.

#### 4. Mismatches do not fail the process with a non-zero exit code

This was the biggest operational surprise.

In the intentional mismatch case:

- `molt verify` still exited with code `0`
- it still printed `verification complete`
- the mismatch only appeared in the structured warning and in the per-table summary counters

That means:

- you cannot treat exit code `0` as "all data matched"
- you must parse the JSON logs or summary counters

For CI or automation, this matters a lot.

### What I think about the tool after trying it

Practical view:

- It is useful.
- It works on a straightforward PostgreSQL -> Cockroach comparison.
- The output is machine-friendly and the row-level mismatch warning is good.
- It feels more like an engineer-oriented verification primitive than a polished one-command migration verdict.

What I like:

- baseline verification was fast and correct
- mismatch reporting was precise
- structured logs are automation-friendly
- active release notes suggest the tool is being maintained

What I do not like:

- preview status still matters
- Docker tag/version consistency was not clean
- local networking was fussier than expected
- the need for `--allow-tls-mode-disable` is another flag to remember
- a mismatch still results in exit code `0`, which is easy to mis-handle in automation

### Bottom line on MOLT Verify

If the question is "does it work at all?", the answer is yes.

If the question is "can I trust it as a migration safety check?", the answer is:

- yes, but only if you read the structured results
- no, not if your automation only checks process exit status

For a real migration pipeline, I would use it, but I would wrap it with my own parser that fails the job when any of these counters are non-zero:

- `num_missing`
- `num_mismatch`
- `num_extraneous`
- `num_column_mismatch`

## Important Artifacts

- Raw requests: `output/requests/`
- Summary: `output/summary.json`
- Seed row counts: `output/sql/01_row_counts.txt`
- Changefeed job status: `output/sql/02_show_changefeed_jobs.txt`
- Receiver logs: `output/sql/03_receiver_logs.txt`
- MOLT Verify summary: `output/molt-verify/summary.json`
- MOLT Verify baseline log: `output/molt-verify/baseline.log`
- MOLT Verify mismatch log: `output/molt-verify/mismatch.log`

## Official References

- CockroachDB `CREATE CHANGEFEED`: https://www.cockroachlabs.com/docs/stable/create-changefeed
- CockroachDB changefeed sinks / webhook sink: https://www.cockroachlabs.com/docs/stable/changefeed-sinks
- CockroachDB message envelopes: https://www.cockroachlabs.com/docs/v26.1/changefeed-message-envelopes
- CockroachDB MOLT Replicator: https://www.cockroachlabs.com/docs/molt/molt-replicator
- CockroachDB MOLT Verify: https://www.cockroachlabs.com/docs/molt/molt-verify
- CockroachDB MOLT releases: https://www.cockroachlabs.com/docs/releases/molt
- CockroachDB migration workflow: https://www.cockroachlabs.com/docs/molt/migrate-load-replicate
- Go package docs for `molt`: https://pkg.go.dev/github.com/cockroachdb/molt
- Cockroach Labs blog on MOLT Verify: https://www.cockroachlabs.com/blog/data-integrity-molt-verify-migrations/
- CockroachDB subreddit demo post: https://www.reddit.com/r/CockroachDB/comments/14fvv29
