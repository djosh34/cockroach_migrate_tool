# Investigation Log

## 2026-04-18 Initial Repository Read

Read:

- `AGENTS.md`
- `investigations/cockroach-webhook-cdc/README.md`
- `investigations/cockroach-webhook-cdc/scripts/run.sh`
- `investigations/cockroach-webhook-cdc/scripts/run-molt-verify.sh`
- `investigations/cockroach-webhook-cdc/receiver/receiver.py`
- `investigations/cockroach-webhook-cdc/docker-compose.yml`
- `investigations/cockroach-webhook-cdc/sql/*.sql`
- `.agents/skills/grill-me/SKILL.md`

Observed repository state:

- the repo is currently investigation-heavy rather than product-heavy
- there is already a runnable CockroachDB + PostgreSQL + webhook receiver setup
- there is already a runnable MOLT verify harness
- the current webhook receiver is only a request capture server and always returns `200`

## 2026-04-18 Baseline Existing Investigation Re-Run

Commands run:

- `./scripts/run.sh` in `investigations/cockroach-webhook-cdc`
- `./scripts/run-molt-verify.sh` in `investigations/cockroach-webhook-cdc`

Baseline findings from the local rerun:

- `initial_scan = 'yes'` on the live changefeed emitted existing rows and then live mutations
- `initial_scan = 'only'` emitted the current state and then completed
- live enriched payloads in the existing setup still lacked clean table metadata until
  `enriched_properties = 'source'` was enabled on the probe feed
- the MOLT verify wrapper proved that MOLT can compare CRDB and PostgreSQL data from this repo setup
- `molt verify` returned exit code `0` even in the intentional mismatch case, so exit
  code alone is not a trustworthy pass/fail signal

Artifacts observed:

- `investigations/cockroach-webhook-cdc/output/sql/04_summary_pretty.json`
- `investigations/cockroach-webhook-cdc/output/sql/02_show_changefeed_jobs.txt`
- `investigations/cockroach-webhook-cdc/output/molt-verify/summary.pretty.json`
- `investigations/cockroach-webhook-cdc/output/molt-verify/mismatch.log`

Specific baseline numbers:

- `/enriched-live`: `337` create events, `6` updates, `2` deletes, `8` resolved messages
- `/snapshot-only`: `333` create events
- `/enriched-source`: `1` update event

Important baseline implication:

- CDC-only initial load plus live catch-up is feasible in principle
- the remaining hard part is not source extraction alone; it is destination-side durable,
  restartable, FK-safe, idempotent apply

## 2026-04-18 Experiment: Multi-Table Source-Enriched Live Feed With Initial Scan

Goal:

- confirm that a multi-table webhook feed can combine:
  - `initial_scan = 'yes'`
  - `envelope = 'enriched'`
  - `enriched_properties = 'source'`

Why this matters:

- without `source.table_name` and `source.primary_keys`, a serious cross-table consumer is much weaker
- the baseline investigation had only proved `enriched_properties = 'source'` on a single-table probe

What was run:

- created a new changefeed on `customers, products, orders, order_items`
- used webhook path `/source-live-initial`
- used `initial_scan = 'yes'`, `enriched_properties = 'source'`, `diff`, `resolved = '2s'`
- then executed additional source mutations including:
  - update on `customers`
  - delete of `orders.id = 8` causing child deletes
  - insert of a new `customer`, `order`, and `order_item`

Artifacts:

- `investigations/cockroach-webhook-cdc/output/requests/*source-live-initial.json`

Findings:

- the combination worked successfully on CockroachDB `v26.1.2`
- row payloads included:
  - `source.database_name`
  - `source.schema_name`
  - `source.table_name`
  - `source.primary_keys`
  - `source.job_id`
- initial scan rows still looked like ordinary creates:
  - `op = "c"`
  - `before = null`
- summary from the captured files:
  - `341` row events
  - `8` resolved messages
  - event mix: `336` creates, `1` update, `4` deletes

Important operational finding:

- this feed shape is sufficiently rich to drive a real consumer
- this is stronger than the earlier baseline and should be considered mandatory for the final design

## 2026-04-18 Experiment: Webhook Retry Semantics On Non-200

Goal:

- verify that CockroachDB retries the same webhook payload when the receiver does not return `200`

Why this matters:

- the proposed design relies on acknowledging only after durable persistence into PostgreSQL migration state

What was built:

- temporary retry probe server:
  - `investigations/cockroach-webhook-cdc/receiver/retry_probe_server.py`
- behavior:
  - first POST for a unique body hash returns `500`
  - second POST for the same body hash returns `200`
  - all requests are logged to `output/retry-probe.log`

What was run:

- created a changefeed for `customers`
- pointed it at `https://host.docker.internal:9443/fail-once`
- updated a customer row

Artifacts:

- `investigations/cockroach-webhook-cdc/output/retry-probe.log`

Findings:

- CockroachDB retried the exact same payload after receiving `500`
- the payload body hash was identical between the failed and successful attempt
- the changefeed therefore behaved as an at-least-once source in practice, not just in theory

Important implication:

- the receiver may safely use HTTP status as its backpressure and correctness signal
- duplicates are normal and must be handled idempotently downstream

## 2026-04-18 Experiment: Resume From Resolved Timestamp Cursor

Goal:

- prove that a stopped changefeed can be recreated from a stored resolved timestamp and recover missed mutations

Why this matters:

- the user requires rerunnable source setup and restartability without replaying everything

Experiment 1:

- created feed `resume-a`
- updated `customers.id = 10` to `resume-step-1`
- captured a resolved timestamp
- canceled the job
- updated the same row again to `resume-step-2`
- created feed `resume-b` with `cursor = <stored_resolved>`

Observation:

- `resume-b` emitted both `resume-step-1` and `resume-step-2`

Interpretation:

- the stored resolved timestamp used in that first attempt was still behind the last emitted event
- recreating from an earlier cursor safely replays already-seen events
- this is acceptable only if the consumer is idempotent

Experiment 2:

- repeated the test more carefully on `customers.id = 11`
- waited until an observed resolved timestamp was greater than or equal to the last emitted event timestamp
- canceled the first job
- performed a second update
- recreated the feed from that later resolved cursor

Artifacts:

- `investigations/cockroach-webhook-cdc/output/requests/*resume-a.json`
- `investigations/cockroach-webhook-cdc/output/requests/*resume-b.json`
- `investigations/cockroach-webhook-cdc/output/requests/*resume2-a.json`
- `investigations/cockroach-webhook-cdc/output/requests/*resume2-b.json`

Findings:

- when the cursor was truly at or after the last applied event timestamp, the recreated feed emitted only the missing later update
- this validates resolved timestamps as the correct restart checkpoint

Important implication:

- the destination must not checkpoint on row receipt alone
- it must advance restart state only at a resolved boundary that it has durably completed

## 2026-04-18 Experiment: `cursor` Plus `initial_scan = 'yes'`

Goal:

- verify that the changefeed can be created with both:
  - an explicit `cursor`
  - `initial_scan = 'yes'`

Why this matters:

- if valid, the source-side setup can provide the destination with an explicit initial scan boundary timestamp instead of forcing inference

What was run:

- captured `SELECT cluster_logical_timestamp()`
- created a webhook changefeed with:
  - `cursor = <captured timestamp>`
  - `initial_scan = 'yes'`
  - `enriched_properties = 'source'`

Finding:

- the statement succeeded on local CockroachDB `v26.1.2`

Important implication:

- the recommended design should record an explicit source cursor at stream creation time
- the destination can treat the first resolved timestamp at or beyond that cursor as the completion of the initial scan window

## 2026-04-18 Experiment: PostgreSQL Apply Strategy Comparison

Goal:

- compare real destination-side apply approaches using actual captured changefeed events

Shared setup:

- used the real `source-live-initial` captured request files
- created fresh PostgreSQL databases using the same target schema
- generated SQL from the captured events using:
  - `investigations/cockroach-webhook-cdc/scripts/render_apply_sql.py`

### Strategy 1: Raw Arrival-Order Apply Into Final Tables

Method:

- replayed every row event in the exact arrival order
- used `INSERT ... ON CONFLICT DO UPDATE` for non-deletes
- used `DELETE ... WHERE pk = ...` for deletes

Artifact:

- `investigations/cockroach-webhook-cdc/output/apply-experiments/apply_direct_run.log`

Result:

- failed

Failure:

- `orders.customer_id = 22` arrived before `customers.id = 22` had been inserted
- PostgreSQL rejected the row due to active foreign keys

Conclusion:

- direct arrival-order apply is not viable with constraints enabled

### Strategy 2: Batch-Local Topological Reorder

Method:

- within each webhook batch:
  - apply upserts ordered by table dependency
  - apply deletes in reverse dependency order

Artifact:

- `investigations/cockroach-webhook-cdc/output/apply-experiments/apply_ordered_run.log`

Result:

- failed

Failure:

- the initial scan spread parent rows across multiple batches
- reordering only within a single batch was insufficient

Conclusion:

- batch-local ordering does not solve initial-scan dependency problems

### Strategy 3: Collapse Raw Events To Final Per-PK State, Then Merge

Method:

- replayed the captured stream into an in-memory per-table, per-PK collapsed state
- emitted final table inserts in dependency order
- loaded the result into PostgreSQL with constraints enabled

Artifacts:

- `investigations/cockroach-webhook-cdc/output/apply-experiments/apply_collapsed_run.log`
- `investigations/cockroach-webhook-cdc/output/apply-experiments/apply_collapsed_molt.log`

Result:

- succeeded
- MOLT verify showed zero mismatches against CockroachDB `demo_cdc`

Conclusion:

- a staging and collapse design is empirically viable
- this is the strongest local evidence gathered in favor of a durable staging architecture

## 2026-04-18 Experiment: PostgreSQL Scoped Role Capabilities

Goal:

- verify what a non-superuser destination role can do in practice

What was set up:

- PostgreSQL role `migrator`
- database `apply_priv_demo`
- helper schema `_cockroach_migration_tool` owned by `migrator`
- grants:
  - `CONNECT`
  - `TEMP`
  - `USAGE` on `public`
  - `SELECT, INSERT, UPDATE, DELETE` on target tables

What was tested as `migrator`:

- create helper table in `_cockroach_migration_tool`
- insert and update target rows via `ON CONFLICT`
- insert progress checkpoints
- create staging table in helper schema
- `COPY ... FROM STDIN` into staging table
- merge staging rows into target tables
- delete from target tables

Artifacts:

- `investigations/cockroach-webhook-cdc/output/apply-experiments/priv_setup.log`
- `investigations/cockroach-webhook-cdc/output/apply-experiments/priv_actions.log`

Finding:

- all tested actions succeeded without superuser privileges

Important implication:

- the final design can avoid superuser assumptions on PostgreSQL if the required grants and helper schema are provisioned correctly

## 2026-04-18 Experiment: CockroachDB Minimal Changefeed Privilege

Goal:

- verify whether a non-admin CockroachDB user can create the webhook changefeed

What was tested:

- created user `cdc_user`
- granted `CHANGEFEED` on `demo_cdc.customers`
- attempted to create a webhook changefeed as `cdc_user`

Artifact:

- `investigations/cockroach-webhook-cdc/output/apply-experiments/source_role_probe.err`

Finding:

- the changefeed was created successfully as the non-admin user in this setup

Important implication:

- the final source-side role requirement can likely be limited to `CHANGEFEED` on the relevant tables, assuming cluster-level prerequisites such as rangefeeds are already enabled

## 2026-04-18 Experiment: Schema Export And Raw Diff

Goal:

- test whether naive textual schema diff is a practical correctness check

What was exported:

- CockroachDB:
  - `SHOW CREATE ALL TABLES`
- PostgreSQL:
  - `pg_dump --schema-only --no-owner --no-privileges`

Artifacts:

- `investigations/cockroach-webhook-cdc/output/schema-compare/crdb_schema.txt`
- `investigations/cockroach-webhook-cdc/output/schema-compare/pg_schema.sql`
- `investigations/cockroach-webhook-cdc/output/schema-compare/raw_diff.txt`

Finding:

- even for semantically matching schemas, raw text diff is noisy and misleading because of:
  - different type names
  - different DDL formatting
  - different constraint and index rendering styles
  - Cockroach-specific clauses like `schema_locked = true`

Important implication:

- the pre-transfer schema checker should be a semantic comparator, not a text diff

## 2026-04-18 MOLT Verify Tooling Observation

Finding:

- `molt verify` returned process exit code `0` even when a deliberate row mismatch existed
- mismatch details appeared in the JSON log stream, not in the exit code

Artifacts:

- `investigations/cockroach-webhook-cdc/output/molt-verify/mismatch.log`
- `investigations/cockroach-webhook-cdc/output/molt-verify/summary.pretty.json`

Important implication:

- the final verification wrapper must parse MOLT logs or summary counts
- it must not rely on process exit code alone

## 2026-04-18 Grill-Me Decision: Per-Database Control State

User decision:

- keep migration state inside each destination database
- do not use a separate central control database

Design impact:

- the recommended design now assumes a helper schema and helper tables inside every migrated destination database
- all atomic progress tracking and merge bookkeeping are local to the destination database being migrated

## 2026-04-18 Grill-Me Decision: API-Level Write Freeze At Handover

User decision:

- PostgreSQL will be kept up to date for an extended period before cutover
- parity can be checked repeatedly with MOLT verify during that period
- at handover time, writes will be blocked at the API layer
- once writes are blocked, CDC is allowed to drain
- if MOLT verify still reports equality, traffic can switch to PostgreSQL

Design impact:

- the recommended cutover protocol should explicitly use an API-level write freeze
- the design does not need to support zero-freeze or continuously writable final handoff
- final handoff correctness can be framed as:
  - block writes
  - wait for lag to reach zero
  - run final verification
  - switch traffic

## Final Design Resolutions

- Helper shadow tables may use an automatically managed minimal primary-key index if needed for performance.
- Reconcile runs continuously.
- Deletes are propagated by PostgreSQL SQL during periodic refresh from helper shadow tables into the real tables.

## Questions Closed By Investigation

- Multi-table `initial_scan = 'yes'` plus `enriched_properties = 'source'` works locally.
- CockroachDB retries the same webhook payload after non-`200`.
- Resolved timestamps are the correct restart checkpoints.
- Arrival-order final-table apply is unsafe with active FKs.
- Batch-local topological reorder is still insufficient for initial scan.
- A staging/collapse approach can reproduce the correct final PostgreSQL state.
- Scoped PostgreSQL roles can handle helper schema state, DML, and `COPY FROM STDIN`.
- Raw schema text diff is not an acceptable schema correctness check.
