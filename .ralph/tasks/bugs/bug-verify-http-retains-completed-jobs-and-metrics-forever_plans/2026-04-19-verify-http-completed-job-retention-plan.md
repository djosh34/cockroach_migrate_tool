# Plan: Bound Verify-Service Completed Job Retention

## References

- Task:
  - `.ralph/tasks/bugs/bug-verify-http-retains-completed-jobs-and-metrics-forever.md`
- Current verify-service code and tests:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/metrics.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `crates/runner/tests/support/verify_image_harness.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This bug is now narrower than the original audit text.
  - `verifyservice/progress.go` no longer exists.
  - Detailed result payloads were already removed from the public HTTP surface.
  - The remaining bug is still real: completed jobs are retained forever in `Service.jobs`, and `/metrics` walks that full history on every scrape.
- No backwards compatibility is required.
  - It is acceptable to stop serving arbitrarily old completed job ids.
  - It is acceptable to tighten job lookup to the active job plus a bounded completed-history window.
- The task markdown plus this plan are sufficient approval for the interface direction in this turn.
  - Do not add a config knob for retention.
  - Do not preserve an unbounded historical job registry.
- Required validation lanes for execution remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` stays out of scope unless execution unexpectedly changes the verify-image harness or another explicit long-lane boundary.
- If the first red slice proves the service still needs multi-job completed-history lookup for an in-scope caller, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- `Service` currently stores all jobs in:
  - `jobs map[string]*job`
  - `activeJobID string`
- `startJob` adds each new job to `jobs`.
- `finishJob` updates terminal state but never prunes old entries.
- `getJobResponse` reads completed jobs from the unbounded map, so completed job ids remain readable forever.
- `metricsStatusSnapshot` iterates the full `jobs` map on every scrape and counts statuses from retained history.
- The current public response is already minimal:
  - `GET /jobs/{job_id}` only returns `job_id` and `status`
- The verify-image harness only depends on:
  - `POST /jobs` returning a `job_id`
  - `GET /jobs/{job_id}` returning `job_id` and `status`
  - it does not depend on indefinite historical lookup

## Improve-Code-Boundaries Focus

- Primary smell: one storage structure is doing the wrong job.
  - `jobs` mixes active execution control, completed-job lookup, and historical metrics accounting.
  - That forces unbounded retention and O(n) metrics collection.
- Preferred cleanup direction:
  - remove the general-purpose `jobs` map entirely
  - model the actual service boundary explicitly:
    - one active job
    - one retained completed job
- This is a better fit for the real public API.
  - The service only allows one active job at a time.
  - The caller only needs to poll the current job through completion.
  - Retaining one terminal job preserves immediate post-completion polling without allowing history to grow forever.
- Secondary cleanup target:
  - if `startedAt`, `finishedAt`, or `failureReason` remain unused after the retention refactor, delete them instead of carrying dead bookkeeping.

## Public Contract After Execution

- `POST /jobs`
  - keeps the current behavior
  - still starts at most one active verify job
  - still returns `job_id` and `status`
- `GET /jobs/{job_id}`
  - returns `200` for the active job id
  - returns `200` for the most recently completed job id
  - returns `404` for older completed job ids once a newer job has finished
- `POST /stop`
  - keeps the current behavior for the active job
- `GET /metrics`
  - keeps the current coarse lifecycle families
  - stops counting arbitrarily old completed jobs
  - reports only the active job plus the one retained completed job
- Explicit retention policy:
  - the service retains at most one completed job
  - finishing a new job evicts the previously retained completed job

## Preferred Design Decision

- Prefer a structural refactor over periodic pruning.
  - a timer-based cleanup loop would add lifecycle complexity for a one-job control plane
  - a size-capped map would still keep a broader job-history abstraction than the service actually needs
- Preferred `Service` shape after execution:
  - `activeJob *job`
  - `lastCompletedJob *job`
- With that shape:
  - `startJob` owns only active-job admission
  - `finishJob` atomically moves the terminal job into `lastCompletedJob`
  - `getJobResponse` checks only two slots
  - `metricsStatusSnapshot` becomes O(1)
- Do not introduce:
  - retention config
  - background janitors
  - a second collection that still preserves unbounded history by accident

## Expected Code Shape

- `cockroachdb_molt/molt/verifyservice/service.go`
  - remove `jobs` and `activeJobID`
  - store the active job directly
  - retain only the most recent completed job
  - simplify stop and lookup helpers around those two slots
- `cockroachdb_molt/molt/verifyservice/metrics.go`
  - stop iterating unbounded job history
  - compute lifecycle counts from `activeJob` and `lastCompletedJob`
- `cockroachdb_molt/molt/verifyservice/http_test.go`
  - add integration-style tests that prove:
    - old completed job ids are evicted
    - `/metrics` no longer reflects unlimited historical completions
  - keep tests on the public HTTP contract only
- `crates/runner/tests/support/verify_image_harness.rs`
  - no change expected
  - only touch this if default validation proves the one-completed-job contract breaks polling

## Type And Boundary Decisions

- Keep `jobStatusView` as the only public job-read DTO.
- Keep `job` private and focused on lifecycle state needed by the service.
- Prefer deleting dead fields over carrying latent result-storage scaffolding.
- Do not expose retention details through API fields or config.
  - the retention rule is an internal policy enforced by behavior:
    - newest completed job remains readable
    - older completed jobs do not

## TDD Execution Order

### Slice 1: Tracer Bullet For Completed-Job Eviction

- [x] RED: add one integration-style HTTP test that starts and completes two jobs, then proves:
  - `GET /jobs/job-000001` returns `404`
  - `GET /jobs/job-000002` still returns `200`
  - `/metrics` reports only one retained completed job instead of two historical completions
- [x] GREEN: make the smallest change that replaces the unbounded job map with a bounded active-plus-last-completed storage boundary
- [x] REFACTOR: remove any compatibility scaffolding left from the old `jobs` map design so the new storage shape is the source of truth

### Slice 2: Verify The Bug Still Holds For Mixed Terminal States

- [x] RED: manually verify whether historical lifecycle counts can still leak through when terminal statuses change across runs; if yes, add one failing integration test proving a newer completed job replaces the previous terminal status in `/metrics`
- [x] GREEN: make lifecycle counting depend only on the retained slots instead of stale historical state
- [x] REFACTOR: keep the metrics snapshot constant-time and owned by the same bounded storage model

### Slice 3: Boundary Cleanup

- [x] RED: run focused `verifyservice` tests again and let the next failure expose any stale assumptions about indefinite completed-job lookup
- [x] GREEN: adapt in-scope tests or callers to the explicit one-completed-job retention boundary without widening the API again
- [x] REFACTOR: delete dead fields or helpers that existed only to support the old unbounded registry

### Slice 4: Repository Validation

- [x] Run `make check`
- [x] Run `make lint`
- [x] Run `make test`
- [ ] Run `make test-long` only if execution unexpectedly changes the long-lane verify-image polling contract

## Expected Boundary Outcome

- The verify-service control plane stops acting like a historical job database.
- Completed-job memory usage becomes constant instead of growing with every finished run.
- `/metrics` scrape cost becomes constant instead of linear in all past jobs.
- The service code gets simpler:
  - one active job slot
  - one completed-job slot
  - no general-purpose historical registry

Plan path: `.ralph/tasks/bugs/bug-verify-http-retains-completed-jobs-and-metrics-forever_plans/2026-04-19-verify-http-completed-job-retention-plan.md`

NOW EXECUTE
