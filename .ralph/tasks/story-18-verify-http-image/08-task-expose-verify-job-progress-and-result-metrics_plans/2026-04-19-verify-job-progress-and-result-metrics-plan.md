# Plan: Expose Verify Job Progress And Result Metrics

## References

- Task:
  - `.ralph/tasks/story-18-verify-http-image/08-task-expose-verify-job-progress-and-result-metrics.md`
- Prior HTTP verify-service tasks:
  - `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs.md`
  - `.ralph/tasks/story-18-verify-http-image/06-task-prove-http-request-inputs-cannot-cause-command-injection-in-verify-execution.md`
  - `.ralph/tasks/story-18-verify-http-image/07-task-route-all-correctness-tests-through-the-verify-http-image-only.md`
- Current verify-service code and tests:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- Existing verify report and metrics ownership:
  - `cockroachdb_molt/molt/verify/inconsistency/reporter.go`
  - `cockroachdb_molt/molt/verify/inconsistency/stats.go`
  - `cockroachdb_molt/molt/verify/rowverify/row_event_listener.go`
  - `cockroachdb_molt/molt/verify/verifymetrics/metrics.go`
- Existing generic Prometheus helper that must not be copied blindly:
  - `cockroachdb_molt/molt/cmd/internal/cmdutil/metrics.go`
- Skill:
  - `tdd`
- Skill:
  - `improve-code-boundaries`

## Planning Assumptions

- This task owns the verify-service `/metrics` surface.
  - It should expose progress and final result state for running and completed verify jobs.
  - It should not add a second metrics server on another port.
- The public verify-service HTTP API remains:
  - `POST /jobs`
  - `GET /jobs/{job_id}`
  - `POST /stop`
  - `GET /metrics`
- The service must stay the source of truth for job progress.
  - Metrics must be rendered from the in-memory job state already owned by `verifyservice`.
  - The implementation must not scrape logs or parse Prometheus text back into state.
- The metrics contract is intentionally job-scoped.
  - `job_id` is a required label even though it increases cardinality.
  - No free-text labels are allowed.
- If the first red slice shows the current verify report stream cannot honestly produce the required source, destination, checked, mismatch, and error counts without inventing fake numbers, execution must switch the plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `verifyservice.Service` currently owns a single in-memory `job` map and a tiny JSON API.
- `job.result` is both:
  - the internal storage model updated during verification
  - the JSON response DTO returned by `GET /jobs/{job_id}`
- That is the main boundary smell for this task.
  - The current shape is append-only and HTTP-oriented.
  - A metrics endpoint needs canonical current totals, not an event log tied to one JSON payload shape.
- `verify.Verify(...)` already emits typed progress through `inconsistency.Reporter`.
  - `StatusReport`
  - `SummaryReport`
  - mismatch objects
  - terminal errors
- The existing `verify/verifymetrics` package is not suitable for the verify-service `/metrics` endpoint.
  - It uses the `molt_verify_` prefix, not `cockroach_migration_tool_verify_`.
  - It uses global `promauto` registration.
  - It has no `job_id` label.
  - It is row-engine oriented, not verify-service job oriented.
- If `/metrics` simply exposes the default Prometheus registry, the service will leak the wrong verify metric family names and fail the task contract.

## Interface And Metric Decisions

- Add one new endpoint:
  - `GET /metrics`
- Keep `/metrics` on the same listener and mux as the rest of the verify-service API.
- Export only verify-service metrics from a service-local registry for this endpoint.
  - Do not expose the process-wide default registry here.
  - Do not expose the legacy `molt_verify_*` metrics on this route.
- Every verify-service metric name must begin with:
  - `cockroach_migration_tool_verify_`
- Use explicit labels only where they add real operator value.
  - required:
    - `job_id`
  - allowed when needed to satisfy the task:
    - `database`
    - `schema`
    - `table`
    - `kind`
    - `status`
  - forbidden:
    - error text
    - info/status message text
    - filter regexes
    - shard identifiers
    - arbitrary command/config strings

## Proposed Metric Families

- `cockroach_migration_tool_verify_job_state`
  - type: gauge
  - labels: `job_id`, `status`
  - semantics:
    - exactly one sample with value `1` for the current state of each known job
    - all other state-label combinations for that job are omitted
- `cockroach_migration_tool_verify_source_rows_total`
  - type: gauge
  - labels: `job_id`, `database`, `schema`, `table`
  - semantics:
    - cumulative source-side row count for the table
- `cockroach_migration_tool_verify_destination_rows_total`
  - type: gauge
  - labels: `job_id`, `database`, `schema`, `table`
  - semantics:
    - cumulative destination-side row count for the table
- `cockroach_migration_tool_verify_checked_rows_total`
  - type: gauge
  - labels: `job_id`, `schema`, `table`
  - semantics:
    - cumulative checked row count for the table
- `cockroach_migration_tool_verify_mismatches_total`
  - type: gauge
  - labels: `job_id`, `schema`, `table`, `kind`
  - semantics:
    - cumulative mismatch totals partitioned by kind
    - initial kind set:
      - `missing`
      - `mismatch`
      - `column_mismatch`
      - `extraneous`
      - `table_definition`
- `cockroach_migration_tool_verify_errors_total`
  - type: gauge
  - labels: `job_id`
  - semantics:
    - cumulative terminal/runtime error count captured for the job

## Derived Count Rules

- The service must not export a vague `rows_todo` metric.
  - Operators should derive remaining work from the clearer source, destination, and checked counts.
- Per-table source row count:
  - derive from the latest `SummaryReport.Stats.NumVerified`
- Per-table checked row count:
  - also derive from the latest `SummaryReport.Stats.NumVerified`
  - that is the current best public meaning of "rows the verifier has checked"
- Per-table destination row count:
  - derive from the latest cumulative row stats:
    - `NumSuccess`
    - `NumConditionalSuccess`
    - `NumMismatch`
    - `NumExtraneous`
  - do not include `NumMissing`, because missing rows do not represent rows found on the destination side
- Per-table mismatch counts:
  - derive from the latest cumulative row stats for row-level kinds
  - derive `table_definition` from the count of recorded `MismatchingTableDefinition` objects for that table
- Per-database row counts:
  - do not store duplicate database-level counters in job state
  - let the metrics collector emit one sample per table with a `database` label
  - database totals are then available by Prometheus aggregation without introducing a second redundant state tree

## Improve-Code-Boundaries Focus

- Primary smell: `jobResult` is currently doing two jobs badly.
  - It is the mutable internal progress state.
  - It is the HTTP JSON response DTO.
  - Metrics would force a third concern onto the same type.
- The fix should be a deeper internal module:
  - create one canonical per-job progress snapshot owned by the service
  - derive both JSON and Prometheus output from that snapshot
- Secondary smell: package-global Prometheus state in `verify/verifymetrics`.
  - The verify-service endpoint must not become a thin wrapper around global `promauto` collectors.
  - `/metrics` should depend on a service-owned collector/registry only.
- Tertiary smell: append-only summaries are the wrong representation for progress.
  - `SummaryReport` events arrive multiple times for the same table over time.
  - Metrics need the latest cumulative totals per table, not a list that can be double-counted.
- Desired end state:
  - one internal job progress model
  - one renderer for job JSON
  - one renderer/collector for Prometheus
  - no duplicate count accumulation hidden inside multiple DTOs

## Proposed Code Shape

- `cockroachdb_molt/molt/verifyservice/service.go`
  - keep handler registration and job lifecycle ownership
  - add `/metrics` route registration
  - stop storing canonical mutable state directly in the HTTP DTO structs
- `cockroachdb_molt/molt/verifyservice/job.go`
  - move job state, job status enum, and response rendering helpers here
  - add canonical per-job progress snapshot types
- `cockroachdb_molt/molt/verifyservice/progress.go`
  - own mutable per-job progress aggregation from `inconsistency.ReportableObject`
  - maintain latest per-table totals keyed by schema/table
- `cockroachdb_molt/molt/verifyservice/metrics.go`
  - own Prometheus descriptor definitions
  - own a custom collector or metrics handler backed by service snapshots
  - own metric-name and label constants
- `cockroachdb_molt/molt/verifyservice/http_test.go`
  - extend with `/metrics` contract tests

## Internal Type Decisions

- Introduce one canonical internal progress type, for example:
  - `jobProgressSnapshot`
- It should contain:
  - `statusMessages []jobStatusMessage`
  - `tableSummaries map[tableKey]tableProgress`
  - `mismatches []jobMismatch`
  - `errors []string`
- `tableProgress` should contain cumulative counts, not raw event history.
  - required fields:
    - `schema`
    - `table`
    - `checkedRows`
    - `sourceRows`
    - `destinationRows`
    - `missingRows`
    - `mismatchRows`
    - `columnMismatchRows`
    - `extraneousRows`
    - `tableDefinitionMismatches`
- `jobResult` may remain as the JSON payload type if needed, but only as a rendered view.
  - It should be created from `jobProgressSnapshot`.
  - It should stop being the canonical mutable storage model.

## Metrics Collection Strategy

- Preferred approach:
  - implement a service-local custom Prometheus collector
  - gather current job snapshots under the service lock
  - emit metric samples during scrape
- Reasons this is the preferred boundary:
  - no separate background metric mutation path
  - no counter-reset problems when jobs finish or the process restarts
  - one canonical state tree for both JSON and metrics
  - no dependency on global default registry behavior
- The collector should emit metrics for:
  - the currently running job
  - completed jobs still retained in memory
- The collector must not emit duplicate samples for repeated progress summaries.
  - it should read only the latest canonical table snapshot for each job/table pair

## TDD Slices

### Slice 1: Tracer Bullet For `/metrics` Exposure

- [x] RED: add one failing `http_test` that starts a job, hits `GET /metrics`, and proves the endpoint exists on the verify-service mux
- [x] GREEN: register `/metrics` on the existing service handler and return Prometheus text
- [x] REFACTOR: keep metrics routing in the service boundary, not in Cobra or a second HTTP server

### Slice 2: Prefix And Registry Isolation

- [x] RED: extend the metrics test to fail if `/metrics` exposes the old `molt_verify_` family or if the new verify-service metrics do not use the `cockroach_migration_tool_verify_` prefix
- [x] GREEN: add a service-local registry/collector so the endpoint exposes only the intended verify-service metrics
- [x] REFACTOR: keep metric descriptors in one place and remove any temptation to reuse default-registry helpers from `cmd/internal/cmdutil`

### Slice 3: Canonical Progress Snapshot Boundary

- [x] RED: add a failing unit/integration test proving repeated `SummaryReport` events for the same table produce one current metrics sample rather than duplicated totals
- [x] GREEN: introduce `jobProgressSnapshot` plus per-table aggregation keyed by schema/table and render JSON from it
- [x] REFACTOR: move mutable progress state out of the HTTP DTO structs

### Slice 4: Running Progress Metrics

- [x] RED: add a failing test that uses a reporting runner to emit in-flight status and summary events, then asserts `/metrics` exposes current `job_state`, `source_rows_total`, and `checked_rows_total` for a running job
- [x] GREEN: update the reporter path so running jobs mutate the canonical progress snapshot as reports arrive
- [x] REFACTOR: keep report-to-progress translation in one aggregator function instead of scattering updates across handlers and finish logic

### Slice 5: Destination Counts And Mismatch Kinds

- [x] RED: add a failing test that emits a summary with success, conditional-success, mismatch, missing, and extraneous counts, then asserts destination rows and mismatch-kind gauges are correct
- [x] GREEN: derive destination rows and mismatch-kind totals from the cumulative row stats plus recorded table-definition mismatches
- [x] REFACTOR: encode the derivation rules once in `tableProgress`, not inline in the metrics renderer

### Slice 6: Completed Job And Error Metrics

- [x] RED: add a failing test that finishes a job with a runtime error and asserts `/metrics` exposes `job_state{status="failed"}` and `errors_total`
- [x] GREEN: surface terminal error counts and final state from the stored finished job snapshot
- [x] REFACTOR: keep terminal state transitions centralized in `finishJob`

### Slice 7: Cardinality And Label Discipline

- [x] RED: add a failing test that asserts the exact allowed labels for each verify-service metric family and that no message text or error text appears anywhere in the metrics exposition
- [x] GREEN: keep labels to `job_id` plus the narrow dimension labels defined in this plan
- [x] REFACTOR: replace ad hoc string literals for label sets and metric names with shared constants

### Slice 8: Validation Lanes

- [x] RED: run `make check`, `make lint`, and `make test`, fixing only the first failing lane at a time
- [x] GREEN: continue until all required lanes pass cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so internal progress ownership is flatter than the current `jobResult`/DTO coupling

## TDD Guardrails For Execution

- One failing test at a time.
- Do not write all `/metrics` assertions first and then all implementation.
- Prefer `http_test.go` integration-style tests that hit the public HTTP surface.
- Do not assert on internal mutexes, private maps, or collector internals.
- Do not parse logs for metrics.
- Do not swallow collector/rendering errors.
  - if the metrics exposition path can fail, that failure must surface honestly in tests and code
- If deriving destination counts from the current typed verify reports proves dishonest or incomplete, switch back to `TO BE VERIFIED` instead of faking semantics.

## Boundary Review Checklist

- [x] `/metrics` is served by the verify-service mux, not a second ad hoc server
- [x] every verify-service metric exposed on `/metrics` uses the `cockroach_migration_tool_verify_` prefix
- [x] the endpoint does not leak the old `molt_verify_*` metric family
- [x] `job_id` is present on every verify-service metric family that represents a job
- [x] the service owns one canonical job progress snapshot separate from the HTTP DTOs
- [x] repeated `SummaryReport` events update current per-table totals instead of being double-counted
- [x] labels stay narrow and explicit; no free-text values leak into Prometheus labels
- [x] operators can infer progress from source, destination, checked, mismatch, and error counts without a separate `rows_todo` metric

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
  Not required unless execution changes the long-lane selection boundary or the task explicitly requires it
- [x] final `improve-code-boundaries` pass after the required lanes are green
- [x] update the task acceptance checkboxes and set `<passes>true</passes>` only after the required lanes pass

Plan path: `.ralph/tasks/story-18-verify-http-image/08-task-expose-verify-job-progress-and-result-metrics_plans/2026-04-19-verify-job-progress-and-result-metrics-plan.md`

NOW EXECUTE
