# Plan: Expose Low-Cardinality Runner Activity, Timing, And Failure Metrics

## References

- Task:
  - `.ralph/tasks/story-22a-runner-metrics/01-task-expose-low-cardinality-runner-activity-timing-and-failure-metrics.md`
- Current runner runtime and HTTP surface:
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/tracking_state.rs`
- Current runner public/runtime contracts and webhook integration tests:
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
  - `crates/runner/tests/support/runner_public_contract.rs`
  - `crates/runner/tests/support/destination_write_failure.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the public interface direction and the highest-priority behaviors to test in this turn.
- This turn re-verified the plan after partial execution work landed in the working tree.
- The next turn should execute from the current partial implementation state rather than restarting slices from zero.
- The public HTTP surface for the runner will become:
  - `GET /healthz`
  - `GET /metrics`
  - `POST /ingest/{mapping_id}`
- `/metrics` must be served by the same TLS listener as the existing webhook surface.
  - No second listener.
  - No separate port.
- The metrics contract must stay intentionally low-cardinality.
  - Allowed labels:
    - `destination_database`
    - `destination_table`
    - `kind`
    - `outcome`
    - `phase`
    - `stage`
  - Forbidden labels:
    - raw error text
    - request path
    - mapping id
    - helper-table name
    - resolved watermark text
    - request body values
- `destination_table` must remain schema-qualified and should come from the existing typed table model, not ad hoc strings.
- If the first RED slice proves the runner cannot expose these metrics honestly without either inventing labels from untrusted strings or duplicating label derivation across webhook and reconcile codepaths, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- The runner already has one shared runtime object:
  - `Arc<RunnerRuntimePlan>` is passed to both webhook and reconcile runtimes.
- The current HTTP server lives only in `webhook_runtime/mod.rs` and exposes:
  - `/healthz`
  - `/ingest/{mapping_id}`
- Webhook apply work and reconcile apply work already pass through stable typed boundaries:
  - webhook row-batch persistence receives `RowMutationBatch`
  - reconcile loops operate on `MappingRuntimePlan` and `HelperShadowTablePlan`
- There is currently no metrics module, no Prometheus registry, and no `/metrics` route in the runner.
- The main boundary smell from `improve-code-boundaries` is obvious:
  - if metrics are added directly in handlers, persistence functions, and reconcile loops, label rendering and metric-family ownership will be spread across three or four modules
  - that would create stringly metrics code and duplicate destination/table label derivation in exactly the places that should stay focused on HTTP routing and SQL work
- The current working tree has already completed most of slices 1 through 4:
  - `crates/runner/src/metrics.rs` now owns the low-cardinality metric families rendered at `/metrics`
  - webhook request metrics and webhook apply timing metrics are already wired
  - reconcile timing metrics are already wired
  - webhook and reconcile contract tests already cover those successful-path surfaces
- The unresolved gap is slice 5 and the reconcile failure semantics behind it:
  - today a reconcile apply failure still bubbles out of `run_reconcile_pass`
  - `reconcile_runtime::serve` then returns a worker error
  - `lib.rs` uses `tokio::try_join!`, so the whole runner exits
  - existing reconcile contract tests currently assert that fatal-exit behavior, which now conflicts with this greenfield task's retry-oriented metrics contract

## Boundary Decision

- Add one dedicated runner metrics module that owns:
  - the registry
  - metric-family definitions
  - Prometheus text rendering
  - the only allowed label shapes for runner metrics
  - typed recording methods for webhook, apply, reconcile, and failure events
- Keep HTTP serving in `webhook_runtime/mod.rs`, but make it a thin boundary.
  - `webhook_runtime` should only route `GET /metrics` to a render call on the shared metrics owner.
  - It must not format Prometheus text itself.
- Keep SQL execution modules focused on database work.
  - `webhook_runtime/persistence.rs` and `reconcile_runtime/*` should emit typed observations into the metrics owner, not build label maps or metric names inline.
- Prefer one shared runtime-owned metrics object reachable from both runtimes.
  - Likely direction:
    - add a `RunnerMetrics` field to `RunnerRuntimePlan`
    - expose a narrow accessor like `runtime.metrics()`
- Reuse existing typed domain data for labels.
  - `destination_database` should come from `PostgresTargetConfig::database()`
  - `destination_table` should come from `QualifiedTableName::label()` / `HelperShadowTablePlan::source_table().label()`
  - `phase` should reuse the existing `ReconcilePhase` domain rather than inventing new strings in each caller
- If needed, introduce tiny typed metric enums for surfaces that do not already have a stable domain type:
  - `WebhookKind`
  - `WebhookOutcome`
  - `ApplyStage`
  - `AttemptOutcome`
- Do not let raw error variants choose label text directly.
  - map runtime failures to bounded enums at the metrics boundary
- Reconcile needs one more boundary cleanup before execution can finish:
  - separate fatal runtime/infrastructure errors from bounded reconcile apply failures
  - bounded apply failures must update tracking state and metrics, then allow the mapping loop to tick again
  - connection/bootstrap/tracking-state persistence failures remain fatal because they mean the runtime cannot safely continue
- Prefer introducing a small typed pass outcome instead of encoding retry semantics through ad hoc `match` branches in `serve`:
  - example direction:
    - `run_reconcile_pass(...) -> Result<ReconcilePassOutcome, RunnerReconcileRuntimeError>`
    - `ReconcilePassOutcome::{Succeeded, ApplyFailedRecorded}`
  - this keeps the retry/fatal boundary explicit and prevents the metrics path from being scattered across the worker loop

## Public Contract To Establish

- `GET /metrics` succeeds on the same TLS listener after bootstrap.
- Every exported metric family name starts with `cockroach_migration_tool_`.
- Webhook request metrics let operators answer:
  - whether requests are arriving at all
  - how many requests arrived over a window
  - when the latest request arrived
- Apply and reconcile metrics let operators compute average duration over a window without histograms.
- Failure metrics let operators answer:
  - whether retries are still happening
  - whether the latest attempt succeeded or failed
  - whether failures are accumulating
- Metric labels remain bounded and schema-qualified.
- The runner does not regain any runtime dependency on the source database in order to produce metrics.

## Proposed Module Shape

- Add `crates/runner/src/metrics.rs`.
- `RunnerMetrics` owns a service-local registry and all metric families.
- `RunnerMetrics` exposes a small typed API, for example:
  - `render() -> String`
  - `record_webhook_request(mapping, kind, outcome, when)`
  - `record_webhook_apply(mapping, table, duration_seconds)`
  - `record_reconcile_apply(mapping, table, phase, duration_seconds)`
  - `record_apply_outcome(mapping, table, stage, outcome, when)`
- Keep per-call label extraction inside the metrics module or in tiny helper methods near it.
  - Callers should pass typed mapping/table/phase context, not prebuilt label strings.
- Consider adding one small metrics-label helper object if it reduces duplication cleanly:
  - `MappingMetricLabels { destination_database }`
  - `TableMetricLabels { destination_database, destination_table }`
- Avoid introducing a second DTO layer if helper methods on existing plan types are enough.
  - The goal is to remove stringly duplication, not create a large metrics-specific type graph.

## TDD Slices

### Slice 1: Tracer Bullet For `/metrics`

- RED:
  - add a webhook integration test proving the runner serves `GET /metrics` over TLS after bootstrap
  - send one successful row-batch request
  - assert the response includes the exact family names and types for:
    - `cockroach_migration_tool_webhook_requests_total`
    - `cockroach_migration_tool_webhook_last_request_unixtime_seconds`
  - assert representative output includes bounded labels:
    - `destination_database="app_a"`
    - `kind="row_batch"`
    - `outcome="ok"`
- GREEN:
  - add the new metrics module
  - wire a service-local registry into `RunnerRuntimePlan`
  - add `GET /metrics`
  - record successful webhook request count and last-request timestamp
- REFACTOR:
  - keep HTTP rendering at the boundary and metric ownership in one module only

### Slice 2: Webhook Outcome Cardinality

- RED:
  - extend the webhook contract test to prove bounded webhook outcomes:
    - malformed payload -> `bad_request`
    - destination write failure -> `internal_error`
    - successful resolved message -> `resolved` + `ok`
  - assert no mapping id or raw error text appears in metric labels
- GREEN:
  - map request failures into bounded `WebhookOutcome`
  - ensure both row-batch and resolved paths record request metrics
- REFACTOR:
  - centralize the request-outcome mapping so the `IntoResponse` path and metrics path cannot drift

### Slice 3: Webhook Apply Duration And Attempt Counters

- RED:
  - add a test that sends a successful row-batch and asserts exact presence of:
    - `cockroach_migration_tool_webhook_apply_duration_seconds_total`
    - `cockroach_migration_tool_webhook_apply_requests_total`
    - `cockroach_migration_tool_webhook_apply_last_duration_seconds`
  - assert `destination_table="public.customers"`
- GREEN:
  - measure row-batch apply duration around `persist_row_batch`
  - record total duration, attempt count, and last duration
- REFACTOR:
  - keep time measurement at the use-case boundary rather than inside raw SQL helpers

### Slice 4: Reconcile Upsert/Delete Timing Metrics

- RED:
  - add or extend reconcile integration coverage so a reconcile cycle produces metrics for:
    - `cockroach_migration_tool_reconcile_apply_duration_seconds_total`
    - `cockroach_migration_tool_reconcile_apply_attempts_total`
    - `cockroach_migration_tool_reconcile_apply_last_duration_seconds`
  - assert bounded `phase="upsert"` and `phase="delete"` labels
- GREEN:
  - measure duration per table per phase in reconcile apply paths
  - record attempts and last duration after each successful apply step
- REFACTOR:
  - reuse the existing `ReconcilePhase` domain rather than inventing separate phase strings

### Slice 5: Failure And Latest-Outcome Metrics

- RED:
  - replace the current fatal-exit reconcile expectations with retry-oriented public contract tests
  - first tracer bullet:
    - induce a reconcile upsert failure
    - assert the runner stays healthy long enough to serve `/metrics`
    - assert exact families and bounded labels for:
      - `cockroach_migration_tool_apply_failures_total`
      - `cockroach_migration_tool_apply_last_outcome_unixtime_seconds`
    - assert reconcile labels:
      - `stage="reconcile_upsert"`
      - `outcome="error"`
  - then add failing-path tests that prove exact families and bounded labels for:
    - `cockroach_migration_tool_apply_failures_total`
    - `cockroach_migration_tool_apply_last_outcome_unixtime_seconds`
  - cover stages:
    - `webhook_apply`
    - `reconcile_upsert`
    - `reconcile_delete`
  - assert both `outcome="success"` and `outcome="error"` gauges can be observed independently
  - assert the runner does not leak raw error text or regress back to fatal exit for bounded apply failures
- GREEN:
  - record success/error timestamps for each stage
  - increment failure counters only on error outcomes
  - make reconcile mapping loops continue after bounded apply failures once the failure has been durably recorded
- REFACTOR:
  - keep the stage mapping in one place so SQL failures do not decide Prometheus label text ad hoc
  - remove any helper or assertion that exists only to prove process death on bounded reconcile apply failures

### Slice 6: Surface-Stability Contract

- RED:
  - add a thin contract assertion that the runner source keeps `/metrics` in the public runtime surface and that metric names remain under the `cockroach_migration_tool_` prefix
- GREEN:
  - codify only the durable surface assertions that protect against accidental removal or prefix drift
- REFACTOR:
  - avoid snapshotting the whole Prometheus text output when a smaller contract states the actual boundary better

## Expected File Touches During Execution

- Runner source:
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/reconcile_runtime/upsert.rs`
  - `crates/runner/src/reconcile_runtime/delete.rs`
  - `crates/runner/src/metrics.rs`
  - possibly `crates/runner/src/tracking_state.rs` if a cleaner outcome hook belongs there
- Runner tests:
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
  - `crates/runner/tests/support/runner_public_contract.rs`
- Test helpers:
  - remove or rewrite helpers that assume reconcile apply failure must terminate the whole runner process
- Build metadata:
  - `Cargo.toml`
  - `crates/runner/Cargo.toml`

## Verified Design Resolution

- The task contract should not be narrowed.
  - This is a greenfield project with no backwards-compatibility requirement.
  - The retry-oriented operator questions in the task are the better contract.
- Therefore execution must change runtime semantics, not the task wording:
  - bounded reconcile apply failures become nonfatal per tick
  - the worker records durable failure state plus metrics and waits for the next interval
  - bounded webhook apply failures already fit the requested metric model and stay as request-scoped failures
  - infrastructure/runtime failures still terminate the runner because continuing would hide a broken runtime
- Existing reconcile tests that assert runner exit on apply failure are now legacy behavior and should be rewritten or removed.
- The next execution turn should start with a RED contract test for nonfatal reconcile failure metrics before touching more implementation.

## Verification Plan

- During execution, follow vertical TDD only:
  - one RED test
  - minimal GREEN implementation
  - immediate refactor
- Verified design decision:
  - reconcile apply failures are no longer treated as task-level fatal behavior
  - execution should convert them into durably recorded, metrics-visible retryable failures
  - only infrastructure/runtime failures remain fatal
- Required end-of-task checks:
  - `make check`
  - `make lint`
  - `make test`
- Do not run `make test-long` unless execution truly changes the ultra-long lane boundary or the task evidence proves it is required.
- Finish by re-reading the implementation through the `improve-code-boundaries` lens:
  - one metrics owner
  - no duplicated string labels
  - no Prometheus text formatting outside the HTTP boundary
  - no scattered metric-family definitions

Plan path: `.ralph/tasks/story-22a-runner-metrics/01-task-expose-low-cardinality-runner-activity-timing-and-failure-metrics_plans/2026-04-20-runner-activity-metrics-plan.md`

NOW EXECUTE
