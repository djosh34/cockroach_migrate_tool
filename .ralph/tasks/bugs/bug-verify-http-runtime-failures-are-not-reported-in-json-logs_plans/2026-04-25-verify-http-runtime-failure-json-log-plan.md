# Plan: Emit Verify HTTP Job Failures Into Structured JSON Logs

## References

- Task: `.ralph/tasks/bugs/bug-verify-http-runtime-failures-are-not-reported-in-json-logs.md`
- Current verify-service runtime and service boundaries:
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/job.go`
  - `cockroachdb_molt/molt/verifyservice/error.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- Current JSON log command surface:
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Current verify HTTP behavior coverage:
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/runtime_test.go`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn is planning-only because the active bug task had no plan artifact yet.
- The task markdown is the approval for this planning turn and defines the required public behavior.
- The bug is a runtime observability boundary bug, not a new HTTP API feature.
  - `GET /jobs/{job_id}` already returns structured failure payloads.
  - JSON logs do not currently emit the same failure facts when a job finishes in `failed`.
- No backwards compatibility is required.
  - If the current logging ownership is split across the CLI and service layers, execution should collapse it to one honest owner instead of adding compatibility shims.
- Required validation lanes for execution remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` stays out of scope unless execution proves this bug changes long-lane selection or an explicit ultra-long contract.
- If the first RED slice proves job-terminal failure logging cannot be added cleanly without duplicate logging paths, secret-leaking string scraping, or another DTO layer that only mirrors `operatorError`, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `cmd/verifyservice/run` logs only two command-owned runtime events in JSON mode:
  - `runtime.starting`
  - `command.failed` when `serviceconfig.Run(...)` itself returns an error
- Per-job failures do not escape `Run(...)`.
  - the HTTP runtime keeps serving after a job failure
  - therefore command-level JSON logging never sees those failures
- The service already classifies terminal job failures correctly:
  - runner source connection failures become `source_access / connection_failed`
  - runner destination connection failures become `destination_access / connection_failed`
  - runner verify execution failures become `verify_execution / verify_failed`
  - mismatch-only completions become `mismatch / mismatch_detected`
- That classification lives at the job/service boundary, but log ownership does not.
  - `Service.finishJob(...)` decides the terminal job status and stores the failure on the job
  - no code at that boundary emits a structured error log event
- This is the main boundary smell.
  - command-layer JSON logging owns startup failures
  - service-layer job lifecycle owns terminal job failures
  - but only the command layer currently writes JSON logs

## Improve-Code-Boundaries Focus

- Primary smell: job failure classification and job failure logging are owned by different layers.
  - the service knows when a job became `failed`
  - the CLI only knows whether the whole runtime process exited
- Preferred cleanup direction:
  - inject logging into the verify-service runtime/service boundary that already owns terminal job state
  - emit one structured error event from that boundary when a job finishes as failed
  - keep the CLI responsible only for command startup/shutdown failures
- Secondary smell: `operatorError` has a JSON payload view, but JSON logging is assembled separately in the CLI package.
- Preferred cleanup direction:
  - reuse one small verify-service-side helper to render logged operator-error fields
  - avoid duplicating category/code/details extraction logic across command and service packages if execution can collapse it
- Bold refactor allowance:
  - if `Service.Dependencies` needs a logger or a narrower terminal-event logger to own job failure logs honestly, add it and delete any now-redundant plumbing
  - if a new helper makes `cmd/verifyservice/writeJSONCommandError(...)` and service-side error logging share one rendering vocabulary, prefer that over separate ad hoc field assembly

## Public Contract After Execution

- In JSON logging mode, every failed verify job must emit an error-level structured log entry when the job reaches terminal failure.
- Required logged fields for failed jobs:
  - `service`
  - `event`
  - `category`
  - `code`
  - `message`
  - `details` when present
- The logged failure payload must match the operator-visible job failure contract already returned by `GET /jobs/{job_id}`.
  - same category
  - same code
  - same message
  - same details
- The log entry must not expose secret material.
  - database passwords in URLs must stay redacted or absent
  - details must continue to use the existing operator-error-safe surface rather than raw config dumps
- Success-path jobs should not emit fake error entries.
- Startup failure logging must keep working as it does today.

## Boundary Decisions

- The service layer should own job-terminal failure logging because it already owns:
  - job lifecycle
  - mismatch-vs-runtime failure classification
  - the final `failure` payload stored on the job
- The CLI command should continue to own:
  - command startup log emission
  - fatal process-level error logging when `Run(...)` returns
- Execution should avoid a muddy split such as:
  - service logs some fields
  - CLI logs a second version of the same job failure
  - runner logs a third partial error string
- Preferred internal shape:
  - `verifyservice` gets either a `zerolog.Logger` dependency or a narrow terminal-event logger dependency
  - `finishJob(...)` or one helper it owns emits the structured failure event exactly once for failed jobs
  - operator-error-to-log-field rendering is centralized

## Files And Structure To Change

- `cockroachdb_molt/molt/verifyservice/service.go`
  - add the logging dependency at the service boundary
  - emit the terminal failed-job log entry exactly once
- `cockroachdb_molt/molt/verifyservice/runtime.go`
  - pass the runtime logger into the service boundary
  - keep startup error behavior unchanged
- `cockroachdb_molt/molt/verifyservice/error.go`
  - optionally add one shared projection/helper if needed so logged operator-error fields and HTTP payload fields do not drift
- `cockroachdb_molt/molt/verifyservice/runtime_test.go`
  - add runtime-level behavior coverage with a real HTTP request and a JSON logger buffer
- `cockroachdb_molt/molt/verifyservice/http_test.go`
  - keep job-result HTTP behavior pinned if execution changes failure ownership or terminal rendering
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - add or adjust command-surface JSON log coverage only if needed to prove the public CLI JSON behavior still works end to end

## Test Strategy

- Prefer public-behavior tests over implementation-only seams.
- The first tracer-bullet test should exercise the runtime through its real HTTP job API while capturing JSON logs from the runtime logger.
  - start the verify-service runtime with a buffer-backed JSON logger
  - submit one job that fails through a real runner boundary
  - assert the logs contain the terminal structured failure fields
- Follow strict vertical TDD.
  - one failing behavior test
  - one minimal green step
  - rerun
  - then add the next failing behavior only if the bug still holds
- Keep secret-safety assertions in the same public tests by using connection-error text that could leak a URL if the boundary is wrong.

## TDD Execution Order

### Slice 1: Tracer Bullet For Source-Access Failure Logging

- [x] RED: add one failing runtime-level test proving that when a job fails with a `source_access / connection_failed` operator error, JSON logs emit an error event containing category, code, message, and details
- [x] GREEN: make the smallest service-boundary change that logs the terminal job failure exactly once
- [x] REFACTOR: centralize operator-error field rendering so the log path and HTTP payload path use the same vocabulary

### Slice 2: Verify-Execution Failures Use The Same Logged Contract

- [x] RED: manually verify whether the bug still holds for `verify_execution / verify_failed`; it did for secret-bearing verify-execution errors, so add one new failing runtime test after the first slice was green
- [x] GREEN: make the minimum change needed so verify-execution failures emit the same structured fields without a second logging path
- [x] REFACTOR: keep failure classification ownership in the existing runner/service boundary rather than string-matching in the logger

### Slice 3: Secret Redaction Stays Honest

- [x] RED: add one failing test proving the logged failure output does not leak a database password or other credential material during verify execution failure rendering
- [x] GREEN: route logged fields through the shared operator-error surface and redact embedded database URI passwords at that boundary
- [x] REFACTOR: remove duplicate raw-error interpolation by making the HTTP payload and log projection share one sanitized operator-error vocabulary

### Slice 4: End-To-End CLI JSON Behavior

- [x] RED: runtime-level tests plus `go test ./cmd/verifyservice` were sufficient, so no extra CLI RED slice was needed
- [x] GREEN: command wiring stayed thin; CLI JSON mode now transports the service-owned failed-job event through the injected runtime logger
- [x] REFACTOR: no separate command-side job-failure formatting logic was introduced

### Slice 5: Manual Verification And Boundary Audit

- [x] Manually verify the original bug report with the supported runtime surface after the test slices are green
- [x] If any failed job class still does not emit the structured log entry, add one new RED test for that gap before continuing
- [x] Do one final `improve-code-boundaries` sweep so startup logging, job-terminal logging, and HTTP failure rendering each have one honest owner

### Slice 6: Repository Validation

- [x] Run `make check`
- [x] Run `make lint`
- [x] Run `make test`
- [x] Do not run `make test-long` unless execution proves this task changed a long-lane boundary

## Expected Boundary Outcome

- Failed verify jobs will be visible in JSON logs and through `GET /jobs/{job_id}` with the same structured operator vocabulary.
- The service layer will own job-terminal failure emission because it already owns the terminal failure classification.
- The CLI layer will stay thin and limited to command/run lifecycle concerns.
- The fix should remove a boundary gap rather than layering more logging code on top of it.

Plan path: `.ralph/tasks/bugs/bug-verify-http-runtime-failures-are-not-reported-in-json-logs_plans/2026-04-25-verify-http-runtime-failure-json-log-plan.md`

NOW EXECUTE
