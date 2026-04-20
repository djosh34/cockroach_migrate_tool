# Plan: Add Cached Shadow-Versus-Real Row-Count And Current Reconcile-State Metrics

## References

- Task:
  - `.ralph/tasks/story-22a-runner-metrics/02-task-add-cached-shadow-vs-real-row-count-and-current-reconcile-state-metrics.md`
- Existing runner metrics boundary:
  - `crates/runner/src/metrics.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
- Existing reconcile state persistence and helper schema:
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/tracking_state.rs`
  - `crates/runner/src/postgres_bootstrap.rs`
- Current contract coverage:
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown already fixes the public metric names, labels, and the highest-priority behavior to test.
- The next turn should execute through vertical TDD slices, one RED contract at a time, not by writing all tests first.
- This is a greenfield project with no backwards-compatibility constraint, so the cleaner boundary wins over preserving accidental internal shapes.
- `/metrics` must stay a pure render of already-collected state.
  - No database queries on scrape.
  - No per-scrape `COUNT(*)`.
- The runner is allowed to spend bounded database work off the scrape path in order to refresh cached metrics.
- If execution reveals that the chosen refresh boundary cannot keep labels typed and bounded without scattering SQL and string rendering across runtime modules, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `RunnerMetrics` already owns the in-memory metrics state and Prometheus text rendering.
- `/metrics` is already routed in `webhook_runtime/mod.rs` and renders only from `runtime.metrics()`.
- Reconcile failures are already nonfatal per pass and persist durable failure state in `_cockroach_migration_tool.table_sync_state`.
- The helper schema already stores exactly the current-state facts needed for two of the new metrics:
  - `last_error`
  - `last_successful_sync_time`
- What does not exist yet:
  - any cached row-count metric family
  - any current-state reconcile gauges
  - any refresh boundary for loading table metrics snapshots from destination state into `RunnerMetrics`
- The main boundary smell from `improve-code-boundaries` is clear:
  - row-count SQL, tracking-state SQL, and Prometheus label rendering would become muddy fast if pushed directly into `metrics.rs`, `/metrics`, or the reconcile loop body
  - the runner needs one typed snapshot object for destination-table metrics and one owner that caches and renders it

## Boundary Decision

- Keep one metrics owner:
  - `RunnerMetrics` remains the only place that owns cached metric values and Prometheus rendering.
- Introduce one typed snapshot-refresh boundary on the worker path:
  - a new helper should query current destination-table observable state
  - it should return typed snapshots, not rendered labels or raw metric names
- Refresh snapshots away from scrape handling:
  - refresh after each reconcile pass attempt, whether the pass succeeds or records a bounded failure
  - this gives bounded `COUNT(*)` work tied to reconcile cadence rather than scrape frequency
- Prefer one snapshot for all needed table facts:
  - shadow row count
  - real row count
  - current reconcile-error flag
  - last-success timestamp
- Keep `tracking_state.rs` focused on helper-schema persistence for stream/reconcile bookkeeping.
  - Do not turn it into a Prometheus-specific module.
  - If the snapshot query needs both helper tables and real tables, a dedicated runtime-side snapshot module is cleaner than overloading `tracking_state.rs`.
- Do not encode metric labels as strings outside the metrics boundary.
  - Callers should pass typed mapping/table/layer snapshot data.

## Public Contract To Establish

- `cockroach_migration_tool_table_rows{destination_database,destination_table,layer}` is exported as a gauge.
- `layer` is bounded to `shadow` or `real`.
- `cockroach_migration_tool_table_reconcile_error{destination_database,destination_table}` is exported as a gauge with:
  - `1` when durable reconcile state currently contains an error for that table
  - `0` when it does not
- `cockroach_migration_tool_reconcile_last_success_unixtime_seconds{destination_database,destination_table}` is exported as a gauge.
- `destination_table` stays schema-qualified.
- Row-count metrics are refreshed from bounded cached state, not by making `/metrics` execute an unbounded recount fan-out.
- Operators can compare shadow-versus-real counts directly for the same table and can also see whether reconcile is currently red and when that table last reconciled successfully.

## Proposed Module Shape

- Extend `crates/runner/src/metrics.rs`:
  - add typed cached metric storage for:
    - `table_rows`
    - `table_reconcile_error`
    - `reconcile_last_success_unixtime_seconds`
  - add typed recording methods such as:
    - `replace_table_snapshot(mapping, snapshot_rows)`
- Add a small typed snapshot module near the runtime path, for example:
  - `crates/runner/src/reconcile_runtime/metrics_snapshot.rs`
- That module should:
  - query the helper schema and mapped destination tables using a live destination connection
  - return `Vec<TableMetricsSnapshot>` for a mapping
- Suggested snapshot shape:
  - `TableMetricsSnapshot { destination_table, shadow_rows, real_rows, has_reconcile_error, last_success_unixtime_seconds }`
- Add a bounded `MetricLayer` enum in `metrics.rs` rather than rendering `"shadow"` and `"real"` ad hoc in multiple places.
- Reuse existing typed table identity:
  - source table labels should still come from `QualifiedTableName::label()` / `HelperShadowTablePlan::source_table().label()`
- Prefer a single refresh entry point from reconcile runtime:
  - after each pass outcome is durably recorded, refresh the mapping snapshot and replace that mapping's cached table-state metrics atomically enough that `/metrics` never mixes unrelated label sets from different tables

## TDD Slices

### Slice 1: Tracer Bullet For Cached Table-State Metrics

- RED:
  - add a reconcile contract test that drives one successful webhook ingest plus one successful reconcile pass
  - assert `/metrics` exposes exact family names and types for:
    - `cockroach_migration_tool_table_rows`
    - `cockroach_migration_tool_table_reconcile_error`
    - `cockroach_migration_tool_reconcile_last_success_unixtime_seconds`
  - assert representative labels:
    - `destination_database="app_a"`
    - `destination_table="public.customers"`
    - `layer="shadow"`
    - `layer="real"`
  - assert the success path shows:
    - identical `shadow` and `real` counts after reconcile
    - reconcile error gauge at `0`
    - last-success gauge present
- GREEN:
  - add cached state fields and rendering for the three new metric families
  - add one snapshot refresh after successful reconcile
- REFACTOR:
  - keep label rendering inside `metrics.rs`

### Slice 2: Bounded Refresh Contract

- RED:
  - add a test that proves metrics are still available before any refresh and that scrape handling itself does not need database work to render
  - add a contract assertion tied to refresh cadence rather than scrape frequency:
    - repeated `/metrics` calls do not change counts unless worker-side state changes
  - prefer asserting stable output across repeated scrapes over implementation-specific query spying
- GREEN:
  - keep `/metrics` as a pure read of cached state
  - refresh only from reconcile/runtime paths, not from the HTTP handler
- REFACTOR:
  - remove any helper that encourages scrape-triggered loading

### Slice 3: Reconcile Error Gauge From Durable State

- RED:
  - extend the existing reconcile failure contract:
    - induce a bounded reconcile failure
    - assert `/metrics` shows `cockroach_migration_tool_table_reconcile_error{...} 1`
    - assert the last-success gauge does not falsely advance on that failing watermark
  - extend the recovery contract:
    - after a successful retry, the same table exposes reconcile error gauge `0`
- GREEN:
  - refresh snapshot state after recorded failure as well as after success
  - map `last_error IS NULL` to `0` and non-null to `1`
- REFACTOR:
  - keep the current-state gauge derived from one snapshot source rather than a separate mutable flag path

### Slice 4: Shadow-Versus-Real Drift Visibility

- RED:
  - add a multi-table reconcile contract showing helper progress ahead of a failed real-table reconcile
  - assert the same metric family exposes both:
    - `layer="shadow"`
    - `layer="real"`
  - assert shadow and real counts can diverge for the failing table while staying schema-qualified and table-specific
- GREEN:
  - refresh counts for every mapped table in the snapshot
  - ensure row counts are stored under the single `table_rows` family keyed by bounded layer labels
- REFACTOR:
  - extract the count-query builder so helper-table names and real-table names are not rendered in multiple places

### Slice 5: Startup Or First-Refresh Semantics

- RED:
  - add a contract test for a runner that starts against existing destination/helper state before the next reconcile tick completes
  - assert the task’s chosen semantics clearly:
    - either metrics stay absent until first bounded refresh
    - or bootstrap performs an initial refresh before serving useful state
- GREEN:
  - implement the chosen semantics with the smallest surface that keeps `/metrics` scrape-safe
- REFACTOR:
  - document the refresh contract in the test name and assertions rather than with comments

## Query Strategy

- Keep the counting work bounded to mapped tables for one mapping refresh.
- Prefer a single snapshot refresh function per mapping over scattered count helpers.
- The snapshot function may issue one count query per layer per table if that keeps the implementation simple and typed.
  - This is acceptable because it is tied to reconcile cadence, not scrape cadence.
- If execution finds a clean way to batch counts without hiding complexity in stringly SQL, that is allowed.
- Do not add approximate counts or stats-table estimates unless the tests and task specifically justify them.

## Expected File Touches During Execution

- Runner source:
  - `crates/runner/src/metrics.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - one new reconcile-side snapshot module, likely `crates/runner/src/reconcile_runtime/metrics_snapshot.rs`
  - possibly `crates/runner/src/lib.rs` or `crates/runner/src/runtime_plan.rs` only if the refresh boundary needs a cleaner shared entry point
- Runner tests:
  - `crates/runner/tests/reconcile_contract.rs`
  - possibly `crates/runner/tests/webhook_contract.rs` if scrape-stability coverage fits better there

## Verification Plan

- Execute strictly as vertical TDD:
  - one RED contract
  - minimal GREEN
  - immediate refactor
- Required end-of-task checks:
  - `make check`
  - `make lint`
  - `make test`
- Do not run `make test-long` unless the implementation actually changes the ultra-long lane boundary or the task proves it is required.
- Final `improve-code-boundaries` pass:
  - one cached metrics owner
  - one typed snapshot-refresh boundary
  - no scrape-path SQL
  - no duplicate label rendering
  - no second metrics DTO layer unless it clearly deletes more complexity than it adds

Plan path: `.ralph/tasks/story-22a-runner-metrics/02-task-add-cached-shadow-vs-real-row-count-and-current-reconcile-state-metrics_plans/2026-04-20-cached-row-count-and-reconcile-state-metrics-plan.md`

NOW EXECUTE
