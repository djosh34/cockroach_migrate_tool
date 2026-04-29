# Plan: Verify-Service HTTP Jobs UX

## References

- Task:
  - `.ralph/tasks/story-35-verify-multi-database-config/02-task-verify-http-jobs-ux.md`
- Prior foundation from task 01:
  - `.ralph/tasks/story-35-verify-multi-database-config/01-task-verify-multi-db-config.md`
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/resolved_config.go`
- Current HTTP/job/runtime seams:
  - `cockroachdb_molt/molt/verifyservice/filter.go`
  - `cockroachdb_molt/molt/verifyservice/job.go`
  - `cockroachdb_molt/molt/verifyservice/result.go`
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/metrics.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
  - `cockroachdb_molt/molt/verifyservice/raw_table.go`
- Tests to extend through public interfaces:
  - `cockroachdb_molt/molt/verifyservice/filter_test.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/runtime_test.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
  - `cockroachdb_molt/molt/verifyservice/result_test.go`
- External contracts to update:
  - `openapi/verify-service.yaml`
  - `docs/operator-guide/verify-service.md`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown already fixes the operator-facing API direction for this turn.
- This turn is planning-only because the task file had no `<plan>` entry and no execution marker.
- No backwards compatibility is allowed.
  - Replace the current single-job UX instead of supporting both old and new response shapes.
  - Remove regex-oriented request fields and old top-level `result` / `failure` response structure once the new schema exists.
- The multi-database config resolver from task 01 is the correct foundation.
  - This task should build on resolved configured databases instead of reintroducing connection details into HTTP.
- A single job may verify multiple configured databases sequentially inside the job worker.
  - The task requires multiple jobs in memory concurrently, but it does not require concurrent per-database execution inside one job.
- If implementation proves the verify engine can report rows or matched objects only through a cleaner seam than the current reporter, execution may reshape the reporter contract, but the public HTTP contract in the task remains fixed.

## Current State Summary

- `Service` is hard-coded to one active job plus one retained job.
  - `activeJob` and `lastCompletedJob` leak storage policy directly into request handling, stop logic, metrics, and lookup.
- `JobRequest` still models single-database regex filters.
  - `include_schema`, `include_table`, `exclude_schema`, and `exclude_table` compile POSIX regexes.
  - Task 02 requires glob matching and a flexible `databases` request shape.
- `job` and `jobResult` are monolithic.
  - They aggregate one verify run into top-level `result` and `failure` fields.
  - Task 02 requires per-database status, timestamps, rows checked, errors, and findings.
- `POST /jobs` returns an accepted envelope, `GET /jobs/{job_id}` returns a different schema, and `GET /jobs` does not exist.
- `VerifyRunner` resolves one configured database from config and runs it once.
  - That is still useful, but it needs a boundary where a multi-database job can invoke it repeatedly with already-resolved concrete selections.
- `metrics.go` assumes at most one active and one completed job.

## Boundary Problem To Flatten

- The main boundary smell is that `Service` currently mixes three concerns in one place:
  - job storage policy
  - job execution lifecycle
  - HTTP response shaping
- A second boundary smell is that `JobRequest` and `RunRequest` combine operator-facing flexible JSON, validation, and runner-facing filter data in one DTO.
- Execution should flatten this by introducing two explicit boundaries:
  - a normalized job plan boundary between HTTP JSON and execution
  - a job store boundary between lifecycle/state management and HTTP handlers/metrics
- This is the `improve-code-boundaries` target for the task.
  - Remove `activeJob` / `lastCompletedJob` special cases entirely.
  - Remove the top-level `result` / `failure` response DTOs instead of bolting new per-database fields onto them.

## Public Contract To Establish

- `POST /jobs`
  - accepts the new flexible request shape:
    - `default_schema_match`
    - `default_table_match`
    - `databases`
  - defaults to verifying all configured databases with `*` matches when the request is `{}`.
  - returns the canonical full job object schema, not a separate accepted envelope.
- `GET /jobs`
  - returns an array of the same job object schema used by `GET /jobs/{job_id}`.
- `GET /jobs/{job_id}`
  - returns one job object from the in-memory store.
- `POST /jobs/{job_id}/stop`
  - stops a running job and returns the same job object schema, with top-level status derived from database statuses.
- Canonical job object fields:
  - `job_id`
  - `status`
  - `created_at`
  - `started_at`
  - `finished_at`
  - `databases`
- Canonical per-database fields:
  - `name`
  - `status`
  - `started_at`
  - `finished_at`
  - `schemas`
  - `tables`
  - `rows_checked`
  - `error`
  - `findings`
- Response rules:
  - no top-level `error`
  - no `mode`
  - no `label`
  - no `current_table`
  - `schemas` / `tables` may be `null` until discovery is known
  - `findings` may be `null` when no findings exist
  - response databases must contain concrete configured database names, not requested glob text
  - matched schemas/tables should become concrete names when known from discovery/reporting
- Status derivation rules:
  - if any database is `running`, the job is `running`
  - if any database is `stopping`, the job is `stopping`
  - if all databases are `succeeded`, the job is `succeeded`
  - if one or more databases are `failed`, the job is `failed`
  - if all unfinished databases were stopped, the job is `stopped`

## Proposed Type Shape

- Replace the regex-oriented request DTOs with a normalized request boundary:
  - `JobCreateRequest`
  - `JobDatabaseRequest`
  - `MatchExpression`
  - `NormalizedJobRequest`
  - `NormalizedDatabaseSelection`
- Normalize flexible JSON once.
  - string-or-array handling for schema/table matchers belongs at the request normalization layer
  - object-vs-string handling for `databases` belongs there too
- Add one resolved execution plan boundary:
  - `ResolvedJobPlan`
    - concrete configured databases selected from config
    - concrete matcher sets per configured database
  - `ResolvedDatabasePlan`
    - configured database name
    - schema globs
    - table globs
    - resolved source/destination connection pair from task 01
- Introduce a real job state model:
  - `JobRecord`
  - `DatabaseJobRecord`
  - `DatabaseJobStatus`
  - `JobStore`
- `JobStore` owns:
  - thread-safe map by job id
  - active cancellation lookup
  - list ordering for `GET /jobs`
  - state transitions and snapshots
- Replace monolithic `jobResult` with per-database accumulation.
  - keep reusable finding rendering helpers
  - attach summaries/findings to the matching `DatabaseJobRecord`
- Split runner-facing execution from HTTP selection:
  - current `RunRequest` should become runner-facing data only
  - multi-database orchestration should loop over `ResolvedDatabasePlan` values
  - each invocation should report into one database-scoped reporter

## Matching And Discovery Rules

- Request matching uses globs, not regexes.
  - validate globs explicitly and fail as `request_validation` before any job is created
  - merge multiple matching request entries for the same configured database
- The runner may still need exact runtime filter configuration for MOLT.
  - if MOLT still accepts regex filters only, convert normalized glob selections into the narrowest safe runtime form inside one adapter layer
  - do not let glob parsing leak into the service or job store layers
- Discovery behavior:
  - configured database names are concrete immediately at job creation
  - `schemas` become concrete once reports reveal matched schemas
  - `tables` become concrete once reports reveal matched tables
  - `rows_checked` stays `null` until a meaningful count is known
  - failed databases should still surface any findings collected before failure

## TDD Slices

### Slice 1: Tracer Bullet For `{}` Expanding To All Configured Databases

- RED:
  - add an HTTP test proving `POST /jobs` with `{}` starts one job that contains all configured database names from config
  - assert the response is the canonical full job object with `running` status and one database entry per configured database
  - assert request bodies no longer use or accept regex filter fields
- GREEN:
  - introduce the new request normalization layer with default `*` behavior
  - replace the accepted envelope with the canonical job response shape
  - add job creation scaffolding for one multi-database job record
- REFACTOR:
  - keep JSON decoding and normalization separate from execution planning

### Slice 2: Flexible Request Shape And Validation

- RED:
  - extend tests for:
    - `databases` as string
    - `databases` as object
    - `databases` as array
    - mixed string/object arrays
    - object entries missing `database_match`
    - invalid glob syntax
    - request matching no configured databases
  - assert errors are `request_validation` and no job is stored
- GREEN:
  - implement one normalization pass plus config-backed matching/merge
- REFACTOR:
  - isolate string-or-array parsing helpers so the rest of the code sees one shape only

### Slice 3: Concrete Response Reality Instead Of Echoed Globs

- RED:
  - add tests proving response `databases` contain concrete configured database names rather than request glob text
  - add tests proving `schemas` and `tables` begin as `null` when not yet known
- GREEN:
  - create `ResolvedJobPlan` and `DatabaseJobRecord` from the normalized request
- REFACTOR:
  - keep plan resolution reusable for POST and future response rendering

### Slice 4: Thread-Safe Multi-Job In-Memory Store

- RED:
  - add service and runtime tests proving two jobs can run concurrently and both remain queryable
  - add tests proving `GET /jobs` returns all current and retained jobs in stable order
  - add tests proving `GET /jobs/{job_id}` and `GET /jobs` use the same object schema
- GREEN:
  - replace `activeJob` / `lastCompletedJob` with `JobStore`
  - add `GET /jobs`
  - make metrics derive counts from the store snapshot
- REFACTOR:
  - move storage/state transition logic out of HTTP handlers

### Slice 5: Per-Database Status Model And Derived Top-Level Status

- RED:
  - add tests covering mixed per-database states and required derived job status rules
  - add tests proving stop responses use the same job object schema
  - add tests proving database-specific failures do not create a top-level error
- GREEN:
  - implement `DatabaseJobStatus`
  - derive top-level status from child database states
  - keep job timestamps coherent as state changes
- REFACTOR:
  - delete obsolete top-level `result` / `failure` response DTOs

### Slice 6: Database-Scoped Findings, Errors, And Row Counts

- RED:
  - extend result/reporting tests and HTTP polling tests to prove:
    - findings land under the matching database entry
    - failures land under the matching database entry's `error`
    - `rows_checked` is present per database and stays `null` when unknown
  - include mismatch and connection-failure paths
- GREEN:
  - add a database-scoped reporter/accumulator
  - move reusable finding rendering into the per-database result layer
- REFACTOR:
  - keep report-to-view mapping out of service handlers

### Slice 7: Runner And Orchestration Boundary Cleanup

- RED:
  - add runner/service tests proving one job can verify multiple configured databases without repeating connection details in the request
  - assert job execution uses config-resolved databases selected by the normalized request plan
- GREEN:
  - split multi-database orchestration from single-database runner execution
  - reuse task-01 resolved config instead of duplicating database resolution logic
- REFACTOR:
  - make the runner consume resolved database work items, not raw HTTP DTOs

### Slice 8: Docs And OpenAPI Move To The New Canonical Schema

- RED:
  - rely on project checks rather than brittle string tests
  - ensure updated examples match the task markdown:
    - `{}` for all databases
    - one string database selector
    - one object database selector
    - mixed array examples
    - `GET /jobs`
    - `GET /jobs/{job_id}`
    - stop flow
- GREEN:
  - update OpenAPI and operator docs to the new request/response contract
- REFACTOR:
  - remove all obsolete single-job-envelope and regex request examples

## Execution Order

- Execute slices strictly in vertical red/green order.
- Start with the HTTP tracer bullet because it fixes the public request/response contract.
- Land the normalization and resolved plan boundary before touching job storage internals.
- Replace storage with `JobStore` before implementing `GET /jobs` and metrics updates.
- Move reporting into per-database state before polishing failure/findings responses.
- Update docs and OpenAPI only after the code contract is stable.

## Verification Gates

- Required before marking the task done:
  - `make check`
  - `make lint`
  - `make test`
- Do not run `make test-long` for this task unless execution proves this task explicitly changed ultra-long coverage.
- Final review must include one explicit `improve-code-boundaries` pass:
  - confirm `activeJob` / `lastCompletedJob` are gone
  - confirm regex request DTOs are gone
  - confirm no top-level job `result` / `failure` schema remains

## Switch-Back Conditions

- Switch this plan back to `TO BE VERIFIED` immediately if:
  - the verify engine cannot support the required per-database progress/finding model without a public contract change
  - glob matching requires a different request shape than the task markdown specifies
  - job retention semantics need a user-visible policy decision not present in the task
  - the new canonical job schema conflicts with an unavoidable runtime limitation discovered during the first RED slice

Plan path: `.ralph/tasks/story-35-verify-multi-database-config/02-task-verify-http-jobs-ux_plans/2026-04-29-verify-http-jobs-ux-plan.md`

NOW EXECUTE
