# Plan: Harden Verify-Service Read Surfaces

## References

- Task:
  - `.ralph/tasks/bugs/bug-verify-http-exposes-job-results-and-metrics-without-auth.md`
- Prior verify-service security and HTTP work:
  - `.ralph/tasks/bugs/bug-verify-http-allows-warning-only-insecure-listener-modes.md`
  - `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs.md`
  - `.ralph/tasks/story-18-verify-http-image/08-task-expose-verify-job-progress-and-result-metrics.md`
- Current verify-service code and tests:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/progress.go`
  - `cockroachdb_molt/molt/verifyservice/metrics.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This bug is a security-boundary correction, not a feature expansion.
  - The verify-service listener is already constrained to HTTPS plus mTLS.
  - The remaining leak is that sensitive verification detail is still part of the public read contract after connection admission succeeds.
- No backwards compatibility is required.
  - It is acceptable to narrow or delete previously-public response fields and metric families.
  - Existing tests that encoded the leaky contract must be rewritten to the new secure contract.
- The repo does not currently have a real service-level authorization seam beyond transport admission.
  - There is no existing principal model, client-common-name allowlist, request identity middleware, or route authorization layer in `verifyservice`.
  - This bug should not invent a second auth subsystem unless the first red slice proves surface reduction cannot keep the contract usable.
- The required validation lanes for this task remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` stays out of scope unless execution changes the long-lane verify-image harness contract.
- If the first red slice proves the public contract cannot stay usable without re-exposing sensitive row/table/job detail, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- `Service.Handler()` publishes four public routes:
  - `POST /jobs`
  - `GET /jobs/{job_id}`
  - `POST /stop`
  - `GET /metrics`
- The sensitive leak today is in the read routes, not the write routes.
  - `GET /jobs/{job_id}` exposes status, timestamps, failure reasons, status messages, table summaries, mismatch details, and runtime errors.
  - `GET /metrics` exposes `job_id`, database names, schema names, table names, mismatch kinds, and error counts.
- The detailed verification payload is stored in service-owned in-memory types:
  - `jobResult`
  - `jobSummary`
  - `jobMismatch`
  - `rowStatsDTO`
  - `jobProgressSnapshot`
- That is the main boundary smell for this bug.
  - Internal verification detail collection and public HTTP rendering are coupled together.
  - The metrics collector then re-exports the same sensitive internal state through Prometheus labels.
- The existing verify-image long-lane harness depends on the current rich `GET /jobs/{job_id}` payload for correctness assertions.
  - That dependence is not a reason to keep the leak.
  - It means execution may need a deliberate harness refactor or long-lane contract adjustment if the default lane reaches that surface.

## Improve-Code-Boundaries Focus

- Primary smell: too-public read models.
  - Sensitive verification detail currently lives in public DTOs and public metric labels.
  - Execution should make the public read surface as small as possible.
- Secondary smell: mixed responsibilities.
  - `jobProgressSnapshot` currently serves three concerns at once:
    - accumulating internal verification detail
    - rendering the public `GET /jobs/{job_id}` payload
    - feeding the public `/metrics` collector
  - These concerns should be split or removed rather than kept in one mutable public-shaped model.
- Tertiary smell: wrong-place observability.
  - Table-level verification diagnostics belong in internal process behavior and tests, not in the unaudited public control-plane surface.
  - Public `/metrics` should not act as a remote dump of internal verify state.
- Security-first cleanup target:
  - remove or collapse public types and metric families whose only purpose is exposing sensitive verification detail
  - prefer deleting a file or entire type over preserving a leaky abstraction

## Public Contract After Execution

- `POST /jobs`
  - stays public
  - continues to return a created job identifier plus running status
- `POST /stop`
  - stays public
  - continues to stop the active job as today
- `GET /jobs/{job_id}`
  - remains available for orchestration and polling
  - must shrink to an explicitly safe status-only contract
  - must not expose:
    - timestamps
    - failure reasons
    - status-message text
    - summaries
    - mismatches
    - errors
    - schema names
    - table names
    - mismatch counts
- `GET /metrics`
  - must stop exposing job-scoped and table-scoped verification detail
  - must not expose labels or values containing:
    - `job_id`
    - database names
    - schema names
    - table names
    - mismatch kinds
    - error text
    - per-job mismatch or error totals
  - safe outcome options:
    - keep `/metrics` with coarse service-level lifecycle gauges/counters only
    - or remove the route entirely if a safe public metrics contract cannot be justified without reintroducing leak paths

## Preferred Design Decision

- Prefer shrinking the public surface over inventing new application-layer auth.
  - The repo already enforces transport-level mTLS.
  - There is no existing principal/authorization boundary to deepen.
  - A new allowlist or identity config would widen config/runtime complexity for one bug.
- Keep the service usable through minimal public orchestration state.
  - callers still need to start jobs, stop jobs, and poll whether a job is still running
  - callers do not need public table-level result detail to satisfy the control-plane contract
- Treat rich verification detail as private implementation state.
  - if it is still needed internally for tests or debugging, it should stop being the public API shape
  - if it is no longer needed at all, delete it instead of carrying dead complexity

## Expected Code Shape

- `cockroachdb_molt/molt/verifyservice/service.go`
  - keep job lifecycle ownership and route registration
  - replace the public `GET /jobs/{job_id}` renderer with a minimal status view
  - stop routing public reads through detailed result DTOs
- `cockroachdb_molt/molt/verifyservice/progress.go`
  - likely shrink heavily or become deletable
  - if retained, it must stop being the public response shape
- `cockroachdb_molt/molt/verifyservice/metrics.go`
  - replace current per-job and per-table metric families with coarse service-level metrics
  - or delete public metrics exposure entirely if no safe coarse contract remains
- `cockroachdb_molt/molt/verifyservice/http_test.go`
  - replace rich-result and rich-metrics assertions with security-focused contract tests
- `crates/runner/tests/support/verify_image_harness.rs`
  - only if default or required validation lanes reach it
  - adapt polling logic to the reduced job-status contract
- `crates/runner/tests/support/e2e_integrity.rs`
  - only if required by the executed lane
  - stop depending on public job-result detail if execution changes the long-lane boundary

## Type And Module Decisions

- Introduce one minimal public read DTO for job polling, for example:
  - `jobStatusView`
- `jobStatusView` should contain only the state needed for polling.
  - minimum:
    - `status`
  - optional:
    - `job_id` if keeping it simplifies callers without widening the leak materially
- Delete or privatize public-shaped detailed result types if they no longer belong in the contract:
  - `jobResult`
  - `jobSummary`
  - `jobMismatch`
  - `rowStatsDTO`
- If detailed progress aggregation is still retained internally, keep it behind a private boundary and do not render it directly over HTTP or Prometheus.
- Remove `sourceDB` and `targetDB` from `Service` if coarse public metrics make them unnecessary.

## Coarse Metrics Direction

- Preferred safe metrics contract:
  - service-level lifecycle metrics only
- Candidate families:
  - `cockroach_migration_tool_verify_active_jobs`
    - gauge
    - no labels
    - value `0` or `1`
  - `cockroach_migration_tool_verify_jobs_total`
    - gauge or counter-style snapshot
    - label: `status`
    - values for `running`, `succeeded`, `failed`, `stopped`
- Explicitly do not keep:
  - `cockroach_migration_tool_verify_job_state`
  - `cockroach_migration_tool_verify_source_rows_total`
  - `cockroach_migration_tool_verify_destination_rows_total`
  - `cockroach_migration_tool_verify_checked_rows_total`
  - `cockroach_migration_tool_verify_mismatches_total`
  - `cockroach_migration_tool_verify_errors_total`
- If the first implementation slice shows even coarse lifecycle metrics are unnecessary or muddy, delete `/metrics` entirely instead of publishing a fake observability surface.

## TDD Execution Order

### Slice 1: Tracer Bullet For The Leaky Job Result Surface

- [ ] RED: add one failing integration test proving `GET /jobs/{job_id}` no longer exposes sensitive result fields after a job runs
- [ ] GREEN: make the smallest change that replaces the public job response with the reduced safe status contract
- [ ] REFACTOR: separate public job polling output from any remaining internal verification-detail storage

### Slice 2: Remove Dead Public Result Shapes

- [ ] RED: run the focused `verifyservice` tests and let the next failure identify which public result types or assertions still assume the leaky contract
- [ ] GREEN: delete or privatize `jobResult`-style DTOs and any response assembly code that no longer belongs in the public API
- [ ] REFACTOR: collapse duplicated storage and rendering paths so the public handler owns only public state

### Slice 3: Tracer Bullet For The Leaky Metrics Surface

- [ ] RED: add one failing integration test proving `/metrics` does not expose `job_id`, database, schema, table, mismatch-kind, or per-job failure detail
- [ ] GREEN: replace the current collector with coarse lifecycle metrics only, or remove `/metrics` if that is the cleaner secure contract
- [ ] REFACTOR: delete the per-job/per-table metrics descriptors and any no-longer-used snapshot helpers

### Slice 4: Boundary Cleanup And Caller Adaptation

- [ ] RED: run the next focused package tests that touch the verify-service polling contract
- [ ] GREEN: adapt any in-scope callers to the reduced status-only job response instead of reintroducing public detail
- [ ] REFACTOR: if long-lane harness code is touched, move correctness expectations off the public job-result surface rather than rebuilding the leak elsewhere

### Slice 5: Repository Validation

- [ ] Run `make check`
- [ ] Run `make lint`
- [ ] Run `make test`
- [ ] Run `make test-long` only if the execution path actually changed the long-lane verify-image harness contract

## Expected Boundary Outcome

- The verify-service public API becomes a real control-plane surface instead of a remote dump of verification internals.
- Public HTTP and public Prometheus output stop carrying table names, database names, mismatch detail, and per-job diagnostic payloads.
- The service internals become simpler:
  - either detailed progress collection is private
  - or it is deleted entirely when it no longer serves a justified boundary
- This bug fix should reduce code, not add a second authorization subsystem without an existing place for it.

Plan path: `.ralph/tasks/bugs/bug-verify-http-exposes-job-results-and-metrics-without-auth_plans/2026-04-19-verify-http-read-surface-hardening-plan.md`

NOW EXECUTE
