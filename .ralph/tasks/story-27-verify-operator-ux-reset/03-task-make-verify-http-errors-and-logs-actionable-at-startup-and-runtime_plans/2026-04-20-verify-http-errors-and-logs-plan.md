# Plan: Make Verify HTTP Errors And Logs Actionable At Startup And Runtime

## References

- Task:
  - `.ralph/tasks/story-27-verify-operator-ux-reset/03-task-make-verify-http-errors-and-logs-actionable-at-startup-and-runtime.md`
- Current Go verify-service boundaries:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/verifyservice/job.go`
  - `cockroachdb_molt/molt/verifyservice/result.go`
  - `cockroachdb_molt/molt/verifyservice/filter.go`
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/result_test.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
- Current CLI/runtime logging boundary:
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Current operator-facing docs and Rust contract consumers:
  - `README.md`
  - `crates/runner/tests/verify_job_result_contract.rs`
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
  - `crates/runner/tests/support/verify_image_harness.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient product approval for this planning turn.
- This turn is planning-only because task 03 had no linked plan artifact or execute marker yet.
- The project is greenfield.
  - no backwards-compatibility shim is needed for the current `{"error":"..."}` payload
  - if the new operator contract lands cleanly, delete or rename stale DTOs instead of translating them forever
- The current failure surfaces are too shallow to be useful:
  - HTTP request failures collapse to one free-text `error` string
  - failed verify jobs return only `job_id` plus `status:"failed"`
  - runtime startup failures log a generic `command.failed` message with no typed boundary
  - verify runner failures are raw lower-level errors with no stable operator category
- The main boundary smell is "typed error boundary, not string buckets".
  - error classification currently lives nowhere
  - rendering is duplicated across HTTP responses, job status, CLI logs, and README examples
  - execution should move that responsibility behind one typed verify-service error contract
- The mismatch path is currently treated as a successful result with mismatch tables inside `result`.
  - this task explicitly calls for mismatch-driven failures to be distinguishable from transport/auth/config/process failures
  - preferred direction is to make mismatches a first-class operator-visible failure category instead of a hidden implication inside `result`
- If the first RED slice proves that mismatch outcomes must stay lifecycle-successful for a stronger existing public contract than the current repo shows, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `service.go` has string-only HTTP error rendering:
  - `writeJSONError` emits only `{"error":"..."}`
  - `writeDecodeJSONError` and most handlers pass raw `err.Error()` through unchanged
  - request-validation failures do not carry stable codes or field-level detail
- `job.go` and `service.go` hide terminal failure causes:
  - successful jobs expose structured `result`
  - failed jobs expose only `job_id` and `status`
  - mismatching table definitions and runtime causes are intentionally stripped from the response today
- `verify_runner.go` knows the true runtime boundary but does not encode it:
  - source connection setup, destination connection setup, verify execution, and close-time errors all collapse into plain `error`
  - operators cannot tell whether the fix belongs to source access, destination access, or verify execution without scraping raw messages
- `runtime.go` and `cmd/verifyservice/verifyservice.go` keep startup failure handling too generic:
  - config validation errors and listener/TLS startup failures are returned or logged as untyped message strings
  - the JSON log format exposes `event` and `message`, but not a stable category/code/details contract
- README and Rust contract tests currently encode the weak public contract:
  - failed final response example is `{"job_id":"job-000001","status":"failed"}`
  - validation example is `{"error":"json: unknown field \"filters\""}`
  - published-image novice coverage expects only the coarse failed status
  - `crates/runner/tests/support/e2e_integrity.rs` currently assumes mismatch detection stays `status:"succeeded"`, so execution must update the mismatch-specific audit helpers to accept the new mismatch terminal contract without weakening the success-path assertions

## Public Contract To Establish

- Keep the start endpoint and polling endpoints from task 02:
  - `POST /jobs`
  - `GET /jobs/{job_id}`
  - `POST /jobs/{job_id}/stop`
- Replace free-text error envelopes with one structured operator contract.
- Preferred HTTP error payload shape:

```json
{
  "error": {
    "category": "request_validation",
    "code": "unknown_field",
    "message": "request body contains an unsupported field",
    "details": [
      {"field": "filters", "reason": "unknown field"}
    ]
  }
}
```

- Preferred completed-job failure shape:

```json
{
  "job_id": "job-000001",
  "status": "failed",
  "failure": {
    "category": "source_access",
    "code": "connection_failed",
    "message": "source connection failed",
    "details": [
      {"reason": "password authentication failed for user verify_source"}
    ]
  }
}
```

- Preferred mismatch-driven terminal shape:
  - use `status:"failed"` with `failure.category:"mismatch"`
  - keep `result` present so the operator still gets mismatch tables and summaries
  - do not force operators to infer "this successful job was actually bad" from nested counters alone
- Preferred operator categories:
  - `config`
  - `startup`
  - `request_validation`
  - `job_state`
  - `source_access`
  - `destination_access`
  - `verify_execution`
  - `mismatch`
  - `cancellation`
- Preferred stable codes are boundary-oriented rather than driver-specific:
  - examples: `invalid_config`, `listener_tls_setup_failed`, `unknown_field`, `invalid_filter`, `job_already_running`, `job_not_found`, `connection_failed`, `verify_failed`, `mismatch_detected`, `job_stopped`
- Logs should reuse the same category/code/message/details vocabulary.
  - do not log one text contract and return a different HTTP contract
  - JSON mode should emit structured fields
  - text mode should still print the human-readable message and the key details

## Improve-Code-Boundaries Focus

- Primary smell to flatten:
  - error classification and rendering are currently scattered across handlers, job rendering, runtime startup, and CLI logging
- Required cleanup during execution:
  - introduce one typed verify-service operator error type instead of ad hoc `err.Error()` plumbing
  - keep HTTP rendering, job terminal rendering, and CLI/runtime logging as thin views over that typed error
  - move runner boundary knowledge into the runner boundary
    - source access failures should be classified in `verify_runner.go`
    - destination access failures should be classified in `verify_runner.go`
    - config/startup failures should be classified near `config.go` and `runtime.go`
  - delete or collapse any DTOs that become pass-through shells once the typed failure contract exists
- Bold refactor allowance:
  - if `job.go` becomes just a thin status wrapper over a richer `jobOutcome` or `jobFailure` type, reshape or merge the files
  - if `writeJSONError` is reduced to a legacy helper, replace it with a typed renderer instead of layering new helpers beside it

## Intended Files And Structure To Add Or Change

- `cockroachdb_molt/molt/verifyservice/error.go`
  - add the typed operator error boundary and JSON/log rendering helpers
- `cockroachdb_molt/molt/verifyservice/service.go`
  - route decode, validation, conflict, not-found, and raw-table failures through the typed error boundary
- `cockroachdb_molt/molt/verifyservice/job.go`
  - add terminal failure payload support and keep final response rendering in one place
- `cockroachdb_molt/molt/verifyservice/result.go`
  - keep mismatch summaries reusable when mismatch becomes a first-class failure category
- `cockroachdb_molt/molt/verifyservice/verify_runner.go`
  - classify source-access, destination-access, and verify-execution failures at the boundary where they occur
- `cockroachdb_molt/molt/verifyservice/runtime.go`
  - classify listener/TLS startup failures instead of returning raw errors
- `cockroachdb_molt/molt/verifyservice/config.go`
  - preserve the specific config reason but normalize it into the typed operator error contract
- `cockroachdb_molt/molt/verifyservice/http_test.go`
  - rewrite failure-oriented HTTP contract tests around the new structured payloads
- `cockroachdb_molt/molt/verifyservice/result_test.go`
  - pin mismatch-result rendering once mismatch becomes an explicit failure category
- `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
  - add boundary-level classification tests for source/destination/verify execution failures
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - log startup/config/runtime failures with structured category/code/details
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - assert JSON logs expose the new typed fields and text mode remains actionable
- `README.md`
  - replace the vague failure and validation examples with the typed contract
  - add at least one runtime/source-access failure example and one validation failure example
- `crates/runner/tests/verify_job_result_contract.rs`
  - keep Rust-side job-result parsing aligned with the new failure/result shape
- `crates/runner/tests/readme_operator_surface_contract.rs`
  - keep README examples locked to the new structured contract
- `crates/runner/tests/novice_registry_only_contract.rs`
  - update the published-image verify expectations for structured failures
- `crates/runner/tests/support/e2e_integrity.rs`
  - adjust `VerifyJobResponse` parsing if terminal failure state now includes `failure` and mismatch-driven failures
- `crates/runner/tests/support/verify_image_harness.rs`
  - adapt to the new terminal failure parsing if the public contract changes

## Vertical TDD Execution Order

### Slice 1: Tracer Bullet For Structured Request-Validation Errors

- [x] RED: add one failing HTTP test that posts an invalid request body and requires:
  - `category`
  - `code`
  - human-readable `message`
  - field-level `details`
- [x] GREEN: implement the minimum typed error boundary needed for JSON decode and validation failures
- [x] REFACTOR: remove or collapse string-only error helpers once the typed renderer exists

### Slice 2: Start-Conflict And Not-Found Errors Use The Same Contract

- [x] RED: add one failing test each for:
  - concurrent `POST /jobs`
  - unknown `GET /jobs/{job_id}` or `POST /jobs/{job_id}/stop`
- [x] GREEN: route job-state conflicts and not-found conditions through the same typed error payload
- [x] REFACTOR: keep job-state error classification in one place instead of repeated handler branches

### Slice 3: Runtime Verify Failures Become Actionable Final Job Responses

- [x] RED: add one failing HTTP test where the runner returns a source-access or verify-execution error and require:
  - terminal `status:"failed"`
  - a `failure` object with stable category/code/message/details
  - the message still carries the real underlying cause instead of a vague label
- [x] GREEN: store typed terminal failure details on the job and render them via `GET /jobs/{job_id}`
- [x] REFACTOR: centralize final job response rendering so runtime failures and mismatch failures do not drift

### Slice 4: Mismatch Detection Becomes A First-Class Failure Category

- [x] RED: add one failing test where verify reports mismatches and finishes without a transport error
- [x] GREEN: return a terminal response that:
  - keeps `result`
  - marks the job as failed for operator purposes
  - exposes `failure.category:"mismatch"` with a message that summarizes the mismatch finding
- [x] REFACTOR: derive mismatch summary data from `jobResult` once, not from duplicate counters in handlers or docs

### Slice 5: Runner Boundary Classification

- [x] RED: add boundary tests in `verify_runner_test.go` that prove:
  - source connection setup failures are classified as `source_access`
  - destination connection setup failures are classified as `destination_access`
  - downstream verify execution failures are classified as `verify_execution`
- [x] GREEN: wrap lower-level errors at the runner boundary with typed operator errors while preserving the real cause text
- [x] REFACTOR: do not add brittle string matching in handlers; the runner should own this classification

### Slice 6: Startup And Config Failures Reuse The Same Vocabulary In Logs

- [x] RED: add failing CLI/runtime tests that require JSON logs for invalid config and runtime startup failure to include:
  - `event`
  - `category`
  - `code`
  - `message`
  - `details`
- [x] GREEN: classify config/load/listener/TLS startup failures and log them through the same typed boundary
- [x] REFACTOR: keep text and JSON logging as two renderings of one error type, not two separate error-classification paths

### Slice 7: README And Rust Contract Coverage

- [x] RED: add failing README and Rust contract assertions for:
  - one structured validation-error example
  - one structured runtime/source-access failure example
  - one mismatch-driven terminal response example
- [x] GREEN: update README and Rust harnesses/parsers to the new public contract
- [x] REFACTOR: keep the README-owned public JSON examples and the Rust parsers aligned through the existing support helpers

### Slice 8: Final Lanes And Boundary Pass

- [x] RED: after the behavior slices are green, run:
  - `make check`
  - `make lint`
  - `make test`
- [x] GREEN: continue until all required default lanes pass cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so the typed error contract, job terminal rendering, and startup logging each have one honest owner

## TDD Guardrails For Execution

- One failing behavior slice at a time.
- Do not add README examples before the underlying HTTP/job/log contract exists.
- Do not keep the old `{"error":"..."}` payload alive behind compatibility branches.
- Do not store only a free-text `error` string on completed jobs.
- Do not classify low-level causes in the HTTP handler when the runner or config/runtime boundary owns that knowledge.
- Do not swallow close-time, startup, or decode errors just because they happen off the happy path.
- If the first mismatch slice proves `status:"failed"` is the wrong lifecycle contract, switch the plan back to `TO BE VERIFIED` immediately instead of smuggling mismatch semantics through vague wording.

## Final Verification For The Execution Turn

- [x] Red/green TDD covers actionable startup/config errors, request-validation errors, runtime verify failures, and mismatch-driven failures
- [x] HTTP error responses expose concrete causes instead of vague status-only output
- [x] Logs include enough structured detail for operators to identify the failing boundary without scraping unrelated debug output
- [x] No verify-service path silently drops, masks, or swallows the underlying failure reason
- [x] Docs and examples include at least one representative validation failure and one representative runtime failure response
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] Do not run `make test-long` unless execution changes long-lane selection or the task explicitly requires it
- [x] One final `improve-code-boundaries` pass after the required default lanes are green

Plan path: `.ralph/tasks/story-27-verify-operator-ux-reset/03-task-make-verify-http-errors-and-logs-actionable-at-startup-and-runtime_plans/2026-04-20-verify-http-errors-and-logs-plan.md`

NOW EXECUTE
