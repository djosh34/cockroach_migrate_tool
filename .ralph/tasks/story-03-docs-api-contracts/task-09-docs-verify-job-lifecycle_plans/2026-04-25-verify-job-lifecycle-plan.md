# Plan: Document Verify Service Job Lifecycle And Stateful Behavior

## References

- Task:
  - `.ralph/tasks/story-03-docs-api-contracts/task-09-docs-verify-job-lifecycle.md`
- Current operator-facing docs:
  - `README.md`
- README operator-surface contract:
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/support/readme_operator_surface.rs`
- Verify service lifecycle/runtime facts:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
- Existing canonical API contract:
  - `openapi/verify-service.yaml`
  - `cockroachdb_molt/molt/cmd/verifyservice/openapi_contract_test.go`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the public documentation direction in this planning turn.
- This turn is planning-only because task 09 had no existing plan artifact.
- The public contract to change is the operator-facing README plus its README contract tests, not the Go runtime API shape.
- The verify API behavior to document already exists and should be described honestly:
  - only one active job may run at a time
  - a second `POST /jobs` while one is running returns `409 Conflict`
  - only the most recent completed job is retained
  - stopping a running job transitions through `stopping` to terminal `stopped`
  - job state is process-local and is lost when the service process restarts
- The current README operator-surface contract is tight:
  - second-level headings must remain only `Setup SQL Quick Start`, `Runner Quick Start`, and `Verify Quick Start`
  - total README word count must stay at or below `1250`
- Current README word count is exactly `1250`, so execution must remove or compress existing verify prose while adding the new lifecycle docs.
- The new lifecycle docs should live under the existing verify section as a third-level heading:
  - `### Job Lifecycle`
  - this satisfies the task requirement without violating the top-level heading contract
- The existing OpenAPI spec already owns the full endpoint catalog and response schemas, so the README should stay focused on operator behavior and lifecycle guidance rather than duplicating the full API contract.
- If the first RED slice proves the required lifecycle section cannot fit inside the README word budget without making the quick start muddy, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - `README.md` contains `### Job Lifecycle` under `## Verify Quick Start`
  - the subsection documents all four terminal/runtime job states in plain language:
    - `running`
    - `succeeded`
    - `failed`
    - `stopped`
  - polling guidance is explicit and includes a concrete interval
  - the docs explain that only one job may run at a time and show the `409 Conflict` response body
  - the docs explain that only the latest completed job is queryable because a new completed job evicts the previous one
  - the docs explain that process restart loses all in-memory job history and old job IDs then return `404`
  - the docs show the full lifecycle with copy-pasteable curl commands and example bodies:
    - start
    - poll while `running`
    - poll once completed
    - inspect result fields
  - the docs tell operators to read `result.summary` first, then `result.mismatch_summary`, then `result.findings`
  - the docs avoid implementation details, future-feature promises, workarounds, auth repetition, and metrics interpretation
- Lower-priority concerns:
  - preserving every current verify quick-start sentence if some of it must be tightened to stay within the README budget
  - keeping lifecycle assertions mixed into one broad verify-section test when a dedicated lifecycle contract test would be cleaner

## Current State Summary

- The README already documents:
  - image pull and config examples
  - `validate-config` and `run`
  - `POST /jobs`, `GET /jobs/${JOB_ID}`, and `POST /jobs/${JOB_ID}/stop`
  - accepted, running, succeeded, stopping, failed, validation-error, and mismatch example bodies
  - a pointer to `openapi/verify-service.yaml`
- The README does not yet document the operator-facing lifecycle rules clearly:
  - no `### Job Lifecycle` subsection
  - no explicit polling interval guidance
  - no explicit one-job concurrency statement
  - no explicit retention/eviction explanation
  - no explicit restart-amnesia explanation
  - no `409 Conflict` example body
  - no guidance for how to read `result.summary`, `result.mismatch_summary`, and `result.findings`
- The Go verify-service tests already prove the important lifecycle facts:
  - `http_test.go` proves `409 Conflict` with structured `job_already_running`
  - `http_test.go` proves completed-job eviction by returning `404` for the older completed job after a newer one finishes
  - `http_test.go` proves stop returns `"status":"stopping"` and later exposes terminal `"status":"stopped"`
  - `service.go` shows job state is held only on the in-process `Service` instance via `activeJob` and `lastCompletedJob`
- The OpenAPI spec and its contract test already cover the canonical API surface, so task 09 should not add another machine-readable API contract layer.

## Boundary Decision

- Keep lifecycle documentation ownership in the README and its operator-surface contract tests.
- Do not add new Go runtime tests for behavior already proven by existing verify-service tests unless a RED slice first exposes a missing public fact.
- Keep the broad verify quick-start contract test focused on:
  - command invocation
  - API entrypoints
  - representative response presence
- Move lifecycle-specific documentation assertions into a dedicated README lifecycle contract test scoped to `### Job Lifecycle`.
- This boundary cleanup reduces one current smell:
  - verify lifecycle semantics are presently implied by scattered examples inside the general quick-start prose
  - they should instead have one explicit subsection and one dedicated docs contract owner

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - operator lifecycle semantics are currently smeared across:
    - scattered README examples
    - one broad verify-section README contract test
    - Go lifecycle tests that are factual sources but not operator docs owners
- Required cleanup during execution:
  - make `README.md` own the lifecycle narrative in one `### Job Lifecycle` subsection
  - make `readme_operator_surface_contract.rs` own lifecycle-doc assertions in one dedicated verify-lifecycle test
  - leave Go tests as the behavioral source of truth, not the owner of operator-facing prose shape
- Bold refactor allowance:
  - if existing verify quick-start prose is redundant with the new lifecycle subsection, delete or merge that prose instead of preserving every sentence
  - if the current broad verify README test becomes muddy, split it into clearer focused tests rather than piling more `contains()` checks into one function

## Intended Public Contract

- Under `## Verify Quick Start`, add `### Job Lifecycle`.
- The subsection should teach the operator-facing lifecycle contract concisely:
  - a short four-state list with plain-language descriptions
  - one explicit polling sentence:
    - `Poll GET /jobs/{job_id} every 2 seconds until status is no longer running.`
  - one explicit concurrency note:
    - only one job can run at a time
    - starting a second job returns `HTTP 409 Conflict`
  - one explicit retention note:
    - only the most recent completed job is retained
    - starting a new job evicts the previous completed job
  - one explicit restart note:
    - job state is held in memory
    - after process restart, previous job IDs return `HTTP 404`
  - one `409 Conflict` response example body matching the current structured operator error envelope
  - one short lifecycle walkthrough using the current quick-start curl flow:
    - start
    - poll while running
    - poll after completion
    - interpret result fields
  - one short result-reading rule:
    - check `result.summary` first
    - then `result.mismatch_summary`
    - then `result.findings`
- The subsection must avoid:
  - mutex/goroutine or in-memory implementation details beyond the operator-facing restart-amnesia fact
  - future persistence promises
  - external storage workarounds
  - Kubernetes-specific restart wording
  - internal Go type names
  - metrics interpretation
  - duplicated auth/TLS explanations already covered elsewhere in the section

## Files And Structure To Add Or Change

- `README.md`
  - add `### Job Lifecycle` under `## Verify Quick Start`
  - add the required state, polling, concurrency, retention, restart, conflict, lifecycle, and result-interpretation guidance
  - compress or remove nearby verify prose as needed to stay inside the README word budget
- `crates/runner/tests/readme_operator_surface_contract.rs`
  - add a dedicated verify lifecycle contract test for the new subsection
  - keep the existing broad verify quick-start contract test focused on the rest of the quick-start surface
- `crates/runner/tests/support/readme_operator_surface.rs`
  - no helper change is expected because `subsection()` already exists
  - only change this helper if execution hits a real parsing limitation

## Vertical TDD Slices

### Slice 1: Tracer Bullet For The New Lifecycle Subsection

- RED:
  - add a failing README operator-surface contract that requires:
    - `### Job Lifecycle` inside `## Verify Quick Start`
    - README word count still at or below `1250`
- GREEN:
  - add the minimal subsection heading and trim nearby verify prose enough to stay inside the word budget
- REFACTOR:
  - keep the new assertion scoped to the subsection rather than the whole README body

### Slice 2: States, Polling, Concurrency, Retention, And Restart

- RED:
  - tighten the lifecycle contract to require:
    - all four job-state descriptions
    - explicit polling interval guidance
    - explicit one-job concurrency note
    - `409 Conflict` wording and example response body
    - completed-job retention and eviction note
    - process-restart `404` note
- GREEN:
  - write concise lifecycle bullets and the conflict example in the new subsection
- REFACTOR:
  - remove or merge older verify bullets that repeat lifecycle facts less clearly

### Slice 3: Full Lifecycle Example And Result Interpretation

- RED:
  - tighten the lifecycle contract to require:
    - start, running poll, completed poll, and result inspection flow
    - `result.summary`
    - `result.mismatch_summary`
    - `result.findings`
    - no forbidden implementation-detail or future-feature phrases
- GREEN:
  - reshape the existing verify examples into one clear lifecycle walkthrough
  - add the short result-reading guidance in the order required by the task
- REFACTOR:
  - keep the README readable by reusing or consolidating existing success/mismatch examples instead of duplicating large blocks

### Slice 4: Final Lanes And Boundary Pass

- RED:
  - after the README lifecycle slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long`
- GREEN:
  - continue until every required default lane passes
- REFACTOR:
  - do one final `improve-code-boundaries` pass and confirm lifecycle-doc ownership is clearer than before

## TDD Guardrails For Execution

- One failing slice at a time.
- Do not bulk-write all lifecycle assertions first.
- Test through public surfaces only:
  - `README.md`
  - `ReadmeOperatorSurface`
  - README operator-surface contract tests
- Do not invent runtime behavior that is not already true in the verify service.
- Do not add new second-level headings.
- Do not grow the README past the enforced `1250`-word ceiling.
- If execution discovers the required lifecycle story cannot stay both accurate and short within the README budget, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Final Verification For The Execution Turn

- [ ] README contains `### Job Lifecycle` under `## Verify Quick Start`
- [ ] All four job states are documented with plain-language descriptions
- [ ] Polling guidance is explicit and includes a concrete interval
- [ ] Concurrency limit and `409 Conflict` behavior are documented with example response
- [ ] Retention policy is stated clearly
- [ ] Restart amnesia behavior is stated clearly without implementation details
- [ ] Full lifecycle example uses actual curl commands and response bodies
- [ ] Result interpretation guidance covers `result.summary`, then `result.mismatch_summary`, then `result.findings`
- [ ] README contains no future-feature promises or persistence workarounds
- [ ] README operator surface contract passes
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long` not run because this is not a story-end task
- [ ] Final `improve-code-boundaries` pass confirms lifecycle docs now have one clear owner in the README contract layer
- [ ] Update the task file and set `<passes>true</passes>` only after all required lanes pass

Plan path: `.ralph/tasks/story-03-docs-api-contracts/task-09-docs-verify-job-lifecycle_plans/2026-04-25-verify-job-lifecycle-plan.md`

NOW EXECUTE
