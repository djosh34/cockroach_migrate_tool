## Task: Make verify HTTP errors and logs actionable at startup and runtime <status>not_started</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Rework verify-service error handling so every configuration, startup, request-validation, and runtime failure tells the operator what failed, why it failed, and where it failed. The higher order goal is to remove the current opaque failure behavior where neither the HTTP response nor the logs explain the real cause.

Current product gap from the 2026-04-20 user review:
- failures currently have “zero explanation”
- the reason is not visible in either logs or HTTP responses
- operators cannot tell whether the failure came from config validation, request validation, source access, destination access, verify execution, or mismatch detection
- the product must not swallow or flatten errors into vague status-only responses

In scope:
- define a structured error contract for verify HTTP responses with stable fields such as category/code/message/details where appropriate
- ensure startup/config failures produce concrete operator-facing error messages rather than generic parse or runtime failures only
- ensure request-validation failures return explicit field-level reasons
- ensure verify execution failures surface the underlying cause in both logs and HTTP-visible job/result state
- ensure mismatch-driven failures are distinguishable from transport/auth/config/process failures
- remove any current code paths that drop, overwrite, or hide meaningful error detail
- update tests and docs to show the new failure contract

Out of scope:
- broad redesign of metrics
- adding persistence for historical errors beyond the existing in-memory job model unless required by the result contract

Decisions already made:
- this project must not swallow or ignore errors; hidden or flattened failures are a defect
- both logs and HTTP responses should become useful first-class operator surfaces
- failure causes should be categorized clearly enough that an operator can tell whether to fix config, credentials, networking, filters, or data mismatches
- this task should revisit any current result or service code that stores only coarse `error` strings or lifecycle states without actionable detail

Relevant files and boundaries:
- `cockroachdb_molt/molt/verifyservice/service.go`
- `cockroachdb_molt/molt/verifyservice/runtime.go`
- `cockroachdb_molt/molt/verifyservice/result.go`
- `cockroachdb_molt/molt/verifyservice/result_test.go`
- `cockroachdb_molt/molt/verifyservice/http_test.go`
- `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
- `crates/runner/tests/verify_job_result_contract.rs`

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers actionable startup/config errors, request-validation errors, runtime verify failures, and mismatch-driven failures
- [ ] HTTP error responses expose concrete causes instead of vague status-only output
- [ ] Logs include enough structured detail for operators to identify the failing boundary without scraping unrelated debug output
- [ ] No verify-service path silently drops, masks, or swallows the underlying failure reason
- [ ] Docs and examples include at least one representative validation failure and one representative runtime failure response
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-27-verify-operator-ux-reset/03-task-make-verify-http-errors-and-logs-actionable-at-startup-and-runtime_plans/2026-04-20-verify-http-errors-and-logs-plan.md</plan>
