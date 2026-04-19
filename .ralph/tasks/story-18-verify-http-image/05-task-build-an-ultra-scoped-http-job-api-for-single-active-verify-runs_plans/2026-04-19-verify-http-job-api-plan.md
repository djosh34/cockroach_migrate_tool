# Plan: Build An Ultra-Scoped HTTP Job API For Single-Active Verify Runs

## References

- Task: `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs.md`
- Prior prerequisite task and plan:
  - `.ralph/tasks/story-18-verify-http-image/04-task-add-verify-service-config-for-source-destination-tls-and-mode-selection.md`
  - `.ralph/tasks/story-18-verify-http-image/04-task-add-verify-service-config-for-source-destination-tls-and-mode-selection_plans/2026-04-19-verify-service-config-plan.md`
- Current verify-service config and command boundary:
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/config_test.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Existing verify execution and report ownership:
  - `cockroachdb_molt/molt/cmd/verify/verify.go`
  - `cockroachdb_molt/molt/verify/verify.go`
  - `cockroachdb_molt/molt/verify/inconsistency/reporter.go`
  - `cockroachdb_molt/molt/verify/inconsistency/row.go`
  - `cockroachdb_molt/molt/verify/inconsistency/table.go`
  - `cockroachdb_molt/molt/verify/inconsistency/stats.go`
- Existing filter boundary that must not leak into the HTTP command:
  - `cockroachdb_molt/molt/cmd/internal/cmdutil/name_filter.go`
  - `cockroachdb_molt/molt/utils/obj_filter.go`
- Existing verify image contract that task 05 will likely need to update:
  - `cockroachdb_molt/molt/Dockerfile`
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/support/verify_docker_contract.rs`
  - `crates/runner/tests/support/verify_image_harness.rs`
- Follow-up tasks:
  - `.ralph/tasks/story-18-verify-http-image/06-task-prove-http-request-inputs-cannot-cause-command-injection-in-verify-execution.md`
  - `.ralph/tasks/story-18-verify-http-image/07-task-route-all-correctness-tests-through-the-verify-http-image-only.md`
  - `.ralph/tasks/story-18-verify-http-image/08-task-expose-verify-job-progress-and-result-metrics.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- This task owns the first real verify-service runtime.
  - It must add `molt verify-service run --config <path>`.
  - It must expose HTTP start/read/stop behavior and execute verify in-process.
- The service stays config-only for connectivity and transport.
  - No request field may change DB URLs, TLS material, listener transport, or verify mode.
  - No `--source`, `--target`, or similar CLI flags may be added to `verify-service run`.
- Only one verify job may run at a time.
  - Completed and stopped jobs remain readable in memory until process restart.
  - A fresh process starts with an empty job history.
- This task should keep the HTTP surface intentionally tiny.
  - Start one job.
  - Read one job by `job_id`.
  - Stop all active work or one active job.
- Metrics are out of scope for this task.
  - Do not add `/metrics` or per-job Prometheus reporting to the service mux.
  - Task 08 will own metrics and richer counts.
- If the first tracer-bullet test shows the proposed request/response contract or package split is wrong, execution must switch back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `verify-service` currently validates config only.
  - There is no `run` command.
  - There is no HTTP server runtime.
  - There is no in-memory job registry.
- The current verify path is still CLI-flag driven under `molt verify`.
  - It owns filter flags through `cmdutil.RegisterNameFilterFlags`.
  - It loads DB connections through `cmdutil.LoadDBConns`.
  - It writes live and final output through `inconsistency.Reporter`.
- The typed verify report surface already exists.
  - `StatusReport`, `SummaryReport`, `MismatchingRow`, `MismatchingColumn`, `MissingRow`, `ExtraneousRow`, and `MismatchingTableDefinition` are the real domain output.
  - The HTTP service should capture these objects directly rather than scraping logs or inventing a second internal result format.
- The current verify image contract still assumes the image entrypoint is the direct `verify` CLI surface.
  - Task 05 likely needs to move the image toward the verify-service runtime while keeping the scratch runtime minimal.
  - Task 07 will later route more correctness tests through that HTTP image boundary.

## Interface And Boundary Decisions

- Add one dedicated runtime command:
  - `molt verify-service run --config <path>`
- Keep Cobra thin.
  - Command code loads validated config and hands off to `verifyservice.Run(...)`.
  - HTTP routing, job lifecycle, TLS listener setup, and verify execution stay out of `cmd/verifyservice`.
- Use three endpoints only:
  - `POST /jobs`
  - `GET /jobs/{job_id}`
  - `POST /stop`
- Keep `POST /jobs` request inputs tightly scoped.
  - No DB or TLS fields.
  - Support only optional schema/table filters.
  - Support both include and exclude filters explicitly, because that is the narrow live-input surface the story decisions already allow.
- Use one stable JSON request shape:

```json
{
  "filters": {
    "include": {
      "schema": "^public$",
      "table": "^(accounts|orders)$"
    },
    "exclude": {
      "schema": "^audit$",
      "table": "^tmp_"
    }
  }
}
```

- Use one stable JSON start response:

```json
{
  "job_id": "job-000001",
  "status": "running"
}
```

- Use one stable JSON status/result response shape:

```json
{
  "job_id": "job-000001",
  "status": "running",
  "started_at": "2026-04-19T18:00:00Z",
  "finished_at": null,
  "failure_reason": null,
  "filters": {
    "include": {
      "schema": "^public$",
      "table": "^(accounts|orders)$"
    },
    "exclude": {
      "schema": "^audit$",
      "table": "^tmp_"
    }
  },
  "result": {
    "status_messages": [],
    "summaries": [],
    "mismatches": [],
    "errors": []
  }
}
```

- Restrict live/final status values to:
  - `running`
  - `succeeded`
  - `failed`
  - `stopped`
- Use explicit HTTP status behavior:
  - `POST /jobs`
    - `202 Accepted` when a job starts
    - `409 Conflict` when another job is already running
  - `GET /jobs/{job_id}`
    - `200 OK` for a known job
    - `404 Not Found` for an unknown job
  - `POST /stop`
    - `200 OK` for stop-all requests, even when nothing is active
    - `200 OK` for a targeted active stop
    - `404 Not Found` for targeted stop of an unknown or non-active `job_id`
- Use process-local monotonic job identifiers.
  - `job-000001`, `job-000002`, ...
  - Deterministic IDs make contract tests stable and avoid UUID noise.
- Keep request and response JSON rendering owned by `verifyservice`.
  - Handlers decode request DTOs and encode response DTOs.
  - The verify engine remains unaware of HTTP.
- Keep the in-memory registry authoritative.
  - One active job pointer plus a map of known jobs.
  - Stopping and state transitions happen only through that registry.

## Improve-Code-Boundaries Focus

- Primary smell: the CLI/global filter boundary is the wrong ownership for the HTTP service.
  - `cmd/internal/cmdutil/name_filter.go` keeps mutable package-global filter state for the legacy CLI.
  - Task 05 should not reuse that global for request-driven filters.
  - The service should own a value-based filter request type and translate it once into the verify layer.
- Secondary smell: verify output is currently bound to logging reporters.
  - The service must not parse log text into JSON.
  - It should add one reporter that captures typed verify events into a job result model and render JSON from that model.
- Tertiary smell: command/bootstrap spaghetti.
  - TLS server wiring, job lifecycle, and handlers do not belong in Cobra command functions.
  - Keep runtime assembly behind a small `verifyservice.Run` or `verifyservice.NewServer`.
- Quaternary smell: duplicate source of truth for job state.
  - Avoid separate "active job", "last result", and "stop state" structs with overlapping fields.
  - One canonical `Job` type should own status, timestamps, filters, captured result, and cancel function.

## Proposed Package Shape

- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - keep `validate-config`
  - add `run --config <path>`
- `cockroachdb_molt/molt/verifyservice/config.go`
  - keep validated config ownership
  - optionally expose listener/TLS helper methods if that keeps runtime bootstrap smaller
- `cockroachdb_molt/molt/verifyservice/service.go`
  - job registry
  - active-job conflict checks
  - job creation and cancellation
  - service startup
- `cockroachdb_molt/molt/verifyservice/http.go`
  - handler registration
  - request decode
  - response encode
- `cockroachdb_molt/molt/verifyservice/job.go`
  - job status enum
  - job/result types
  - monotonic job id allocation
- `cockroachdb_molt/molt/verifyservice/reporter.go`
  - typed `inconsistency.Reporter` implementation that records status/summaries/mismatches/errors
- `cockroachdb_molt/molt/verifyservice/filter.go`
  - request filter DTOs
  - regex compilation and include/exclude matching
  - adapter into the verify layer

## Verify Result Ownership

- The job result should preserve structured verify output, not only a final boolean.
- The capture reporter should append:
  - status messages from `inconsistency.StatusReport`
  - row/table summaries from `inconsistency.SummaryReport`
  - table-definition mismatches
  - row mismatches
  - missing rows
  - extraneous rows
  - column mismatches
- Runtime errors should be stored separately from mismatches.
  - Fatal `verify.Verify(...)` errors set:
    - job status to `failed`
    - `failure_reason` to the error string
    - an `errors` entry in the result payload
- Stopped jobs should not pretend they failed.
  - cancellation through `/stop` sets job status to `stopped`
  - `failure_reason` should explain that the job was stopped by request

## Filter Contract Decision

- Support four optional regex inputs:
  - `filters.include.schema`
  - `filters.include.table`
  - `filters.exclude.schema`
  - `filters.exclude.table`
- Missing filters mean "match everything" for include and "exclude nothing" for exclude.
- Bad regex input is a request error.
  - `POST /jobs` should return `400 Bad Request` when a filter regex does not compile.
- Do not allow arbitrary SQL fragments, column filters, or any request inputs beyond these regex fields.
- Do not reuse the `cmdutil` package-global filter state.
  - The HTTP service needs request-scoped filters, not command-scoped globals.
- If execution shows the current verify layer can only support include filters without muddy adapter code, switch this plan back to `TO BE VERIFIED` instead of sneaking in a half-shape contract.

## TLS And Auth Runtime Decision

- `verify-service run` must honor the existing validated config from task 04.
- If `listener.transport.mode` is `http`, start a plain `http.Server`.
- If `listener.transport.mode` is `https`, start TLS using the configured server cert/key.
- If `listener.client_auth.mode` is `mtls`, configure the TLS server to require and verify client certs using `client_ca_path`.
- If direct service auth is disabled, log a clear startup warning.
  - Keep the explicit "no extra built-in protection" wording already established in task 04.

## Verify Image Contract Impact

- Task 05 likely changes the verify image's public entrypoint.
  - The image should move from direct `molt verify` execution toward the verify-service command surface.
- Keep the scratch runtime minimal.
  - No shell wrapper.
  - No extra runtime payload.
- Update the Rust image-contract tests only as needed for the new public surface.
  - The image should still stay rooted in the Go verify slice.
  - Task 07 will own broader correctness-test rerouting, so task 05 should only update the public contract needed for the new HTTP service surface.

## Files And Structure To Add Or Change

- [ ] `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - add `run --config <path>` and keep command bootstrap thin
- [ ] `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - command-surface tests for `run` help and config-only behavior
- [ ] `cockroachdb_molt/molt/verifyservice/service.go`
  - server/bootstrap plus single-active job registry
- [ ] `cockroachdb_molt/molt/verifyservice/http.go`
  - handlers and JSON contracts
- [ ] `cockroachdb_molt/molt/verifyservice/job.go`
  - canonical job/result types and status enum
- [ ] `cockroachdb_molt/molt/verifyservice/reporter.go`
  - typed verify-result capture
- [ ] `cockroachdb_molt/molt/verifyservice/filter.go`
  - request filter parsing/validation without `cmdutil` globals
- [ ] `cockroachdb_molt/molt/verifyservice/*_test.go`
  - public-behavior tests for start/get/stop, restart-loss, filters, and result capture
- [ ] `cockroachdb_molt/molt/Dockerfile`
  - update the public verify-image surface only if runtime entrypoint must move for this task
- [ ] `crates/runner/tests/verify_image_contract.rs`
  - update the verify image public-contract assertions only if the entrypoint/help output changes
- [ ] `crates/runner/tests/support/verify_docker_contract.rs`
  - keep scratch/minimal-runtime assertions aligned with the new verify-service surface
- [ ] docs or README snippets that explicitly mention direct-auth-disabled deployment expectations, only if task 05 introduces a new user-facing runtime path that needs it

## TDD Execution Order

### Slice 1: Tracer Bullet For The Service Runtime

- [x] RED: add one failing integration-style Go test that boots the service with `httptest`, `POST /jobs`, and gets back `202` plus a stable `job_id`
- [x] GREEN: add the smallest handler + job-registry path needed to create one running job record
- [x] REFACTOR: keep HTTP decode/encode in `verifyservice`, not in Cobra

### Slice 2: Single-Active Conflict Contract

- [x] RED: add one failing test proving a second `POST /jobs` while the first job is still running returns `409 Conflict`
- [x] GREEN: add the smallest active-job guard in the registry
- [x] REFACTOR: keep active-job ownership in one place instead of spreading mutex checks across handlers

### Slice 3: Live Status Lookup And Stable Job Identity

- [x] RED: add one failing test that `GET /jobs/{job_id}` returns `running` while the job is still in progress and that repeated reads return the same job's latest state
- [x] GREEN: persist known jobs in memory and expose a stable status/result DTO
- [x] REFACTOR: ensure `GET` uses the registry model directly rather than rebuilding state ad hoc

### Slice 4: Final Success/Failure Result Capture

- [x] RED: add one failing test using a stubbed verify runner that records mismatches and a final outcome, then assert `GET /jobs/{job_id}` returns the final JSON payload with `succeeded` or `failed`, mismatches, and `failure_reason` when relevant
- [x] GREEN: add a capture reporter and runtime completion path
- [x] REFACTOR: keep JSON rendering as a projection of typed captured events rather than string parsing

### Slice 5: Stop-All And Targeted Stop

- [x] RED: add one failing test for `POST /stop` with no `job_id` and one for targeted stop with a `job_id`
- [x] GREEN: wire request cancellation through the job registry and running job context
- [x] REFACTOR: make "stop all" and "stop one" call the same registry cancellation path where possible

### Slice 6: Unknown Or Non-Active Targeted Stop

- [x] RED: add one failing test that `POST /stop` with an unknown or already-finished `job_id` returns `404`
- [x] GREEN: distinguish active and historical jobs in the registry
- [x] REFACTOR: keep "known historical result" separate from "currently stoppable"

### Slice 7: Restart-Loss Behavior

- [x] RED: add one failing test that starts a job or stores a finished result in one service instance, then creates a fresh service instance and confirms the old `job_id` is gone
- [x] GREEN: keep job state purely in-memory with no persistence hooks
- [x] REFACTOR: delete any accidental persistence abstraction if it appears during implementation

### Slice 8: Scoped Filter Inputs

- [x] RED: add one failing test that `POST /jobs` accepts valid include/exclude schema/table filters, rejects bad regex with `400`, and does not allow unrelated fields to affect connection behavior
- [x] GREEN: add request-scoped filter parsing and translation into the verify path
- [x] REFACTOR: flatten the filter boundary so service requests do not depend on mutable `cmdutil` globals

### Slice 9: TLS And Service Bootstrap

- [x] RED: add one failing command/runtime test for `verify-service run --config <path>` that proves the service honors the validated listener mode and emits the explicit direct-auth warning when auth is disabled
- [x] GREEN: add the smallest runtime bootstrap for HTTP/HTTPS/mTLS mode selection
- [x] REFACTOR: keep TLS/server bootstrap in `verifyservice`, not in the command package

### Slice 10: Verify Image Public Surface

- [x] RED: run the smallest Rust contract or Docker contract that fails first if the verify image public surface changed
- [x] GREEN: update the image contract only as far as task 05 requires
- [x] REFACTOR: keep the image minimal and verify-slice-owned after the service entrypoint change

### Slice 11: Local Go Validation

- [x] RED: run focused Go tests for the new `verifyservice` package and command, fixing the first failure at a time
- [ ] GREEN: end with `go test ./...` from `cockroachdb_molt/molt`
  This still fails in the current machine environment because legacy upstream Go packages expect local CockroachDB/MySQL services that are not part of the repository gate; the task-specific `verifyservice`, `cmd/verifyservice`, and `utils` packages are green.
- [x] REFACTOR: if execution leaves command wiring, filter parsing, and result capture tangled together, flatten that boundary before moving on

### Slice 12: Repository Validation Lanes

- [x] RED: run `make check`, `make lint`, and `make test`, fixing only the first failing lane at a time
- [x] GREEN: continue until all required lanes pass cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass focused on removing global filter state and keeping HTTP/runtime ownership inside `verifyservice`

## TDD Guardrails For Execution

- Start with one failing public-behavior test per slice.
- Do not write all handler tests first and all runtime code second.
  - Vertical slices only.
- Prefer `httptest` or direct handler tests over brittle CLI subprocess tests for the HTTP contract.
- Keep the verify engine behind a narrow injected runner seam for tests.
  - The seam should model behavior, not private implementation.
- Do not parse logs to produce HTTP JSON.
- Do not create a second config plane through HTTP or CLI flags.
- Do not accept extra request fields "for future use".
- Do not add a metrics endpoint in this task.
- Do not swallow cancellation or runtime errors.
  - They must become explicit job status and error payload.
- Do not run `make test-long` unless execution changes ignored-test selection or the task explicitly proves it is required.

## Boundary Review Checklist

- [x] `verifyservice` owns HTTP routing, job lifecycle, and JSON rendering
- [x] Cobra only loads config and starts the runtime
- [x] Request filters are request-scoped values, not `cmdutil` globals
- [x] The HTTP API cannot change DB URLs, TLS material, listener auth, or verify mode
- [x] Single-active-job semantics are enforced by one registry, not handler-local checks
- [x] Job status is limited to `running`, `succeeded`, `failed`, and `stopped`
- [x] Captured verify output comes from typed report objects, not parsed log lines
- [x] Job history stays in memory only and disappears on restart
- [x] The verify image remains scratch-based and verify-slice-owned if its public surface changes

## Final Verification For The Execution Turn

- [x] focused Go tests for new `verifyservice` runtime slices
- [ ] `go test ./...` from `cockroachdb_molt/molt`
  This fails outside the task slice because the vendored legacy Go tests still expect local CockroachDB/MySQL services on this machine; it is not one of the required task completion gates.
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] if execution changes ignored-test selection or the task explicitly requires it: `make test-long`
  Not required; this task did not change the ignored long-lane selection boundary.
- [x] final `improve-code-boundaries` pass after all required lanes are green
- [x] update the task acceptance checkboxes and set `<passes>true</passes>` only after the required lanes pass

Plan path: `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs_plans/2026-04-19-verify-http-job-api-plan.md`

NOW EXECUTE
