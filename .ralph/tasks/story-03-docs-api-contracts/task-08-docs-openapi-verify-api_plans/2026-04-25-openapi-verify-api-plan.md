# Plan: Create OpenAPI 3.0 Specification For Verify Service HTTP API

## References

- Task:
  - `.ralph/tasks/story-03-docs-api-contracts/task-08-docs-openapi-verify-api.md`
- Verify HTTP implementation:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/error.go`
  - `cockroachdb_molt/molt/verifyservice/raw_table.go`
  - `cockroachdb_molt/molt/verifyservice/job.go`
- Verify HTTP contract tests:
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/runtime_test.go`
- Default verification lanes that must stay honest:
  - `Makefile`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Current operator docs contract:
  - `README.md`
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/support/readme_operator_surface.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient product approval for this planning turn.
- This turn is planning-only because task 08 had no linked plan artifact yet.
- The OpenAPI file must describe the public HTTP contract honestly as proven by the current Go handlers and tests.
- `make test` only runs:
  - `cargo test --workspace`
  - `go test ./cmd/verifyservice -count=1`
- Because of that lane shape, the OpenAPI validation contract must live in the Go command package test lane or it will not actually run in default verification.
- The root `README.md` is already at `1249` words, so adding the required OpenAPI reference means execution must remove or compress at least a little existing prose to keep the operator-surface contract green.
- Current error payloads are not fully uniform:
  - job endpoint decode and state errors use structured operator errors
  - `/tables/raw` currently returns plain `{ "error": "..." }` bodies for its `400` and `403` cases
- If the first RED slice proves the task truly requires one single structured `400` schema across every verify endpoint, this plan is wrong and must be switched back to `TO BE VERIFIED` before execution continues.
- If the first RED slice proves the chosen OpenAPI validation library cannot be exercised cleanly from `go test ./cmd/verifyservice` without turning the command package test lane into a muddy integration bucket, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - the OpenAPI file exists at a stable repo-root location and validates as OpenAPI 3.0
  - every verify-service path is present:
    - `POST /jobs`
    - `GET /jobs/{job_id}`
    - `POST /jobs/{job_id}/stop`
    - `POST /tables/raw`
    - `GET /metrics`
  - request bodies match the real decoder contract:
    - flat job filter fields
    - empty object for stop
    - raw-table request with `database`, `schema`, `table`
  - response schemas and examples match the real handler behavior, including the current split between operator-error and plain-error payloads
  - the README points operators to the OpenAPI file without breaking the current short quick-start contract
- Lower-priority concerns:
  - preserving every current README sentence if one sentence must be tightened to afford the OpenAPI reference
  - keeping all spec assertions inside one giant existing CLI/runtime test file

## Current State Summary

- The verify service already exposes the public API surface we need to document:
  - `POST /jobs` starts one job and returns `202`
  - `GET /jobs/{job_id}` returns `200` for running and completed jobs, and `404` for unknown jobs
  - `POST /jobs/{job_id}/stop` returns `200` with `"status":"stopping"` and `404` for unknown jobs
  - `POST /tables/raw` returns `200` when enabled, `403` when disabled, and `400` for invalid raw-table requests
  - `GET /metrics` returns Prometheus text
- The handler layer already proves the important schema facts:
  - `job_id` is a string path parameter
  - `POST /jobs` accepts only flat top-level filters:
    - `include_schema`
    - `include_table`
    - `exclude_schema`
    - `exclude_table`
  - unknown top-level fields are rejected
  - multiple JSON documents are rejected
  - oversized bodies are rejected with `413`
- Completed job responses are richer than the current README examples:
  - success responses may include `result.summary`, `result.table_summaries`, `result.findings`, and `result.mismatch_summary`
  - failed responses may include both `failure` and `result`
  - stopped jobs return `"status":"stopped"`
- The API is intentionally stateful:
  - only one active job can run at a time
  - only the most recent completed job is retained
  - results are lost on process restart
- There is no existing OpenAPI file or default-lane contract that validates one.
- The current README verify quick-start documents the basic curl flow but does not point to an authoritative full API contract file.

## Boundary Decision

- Keep the OpenAPI contract owned by a dedicated Go test in the verify command package lane.
- Do not bury the new spec assertions inside `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`, which is already focused on CLI/runtime behavior.
- Keep the README ownership where it already belongs:
  - `crates/runner/tests/readme_operator_surface_contract.rs` should only prove that the verify quick-start references the canonical OpenAPI file location
  - it should not re-assert path-by-path API schema details that belong in the spec contract
- Preferred structure during execution:
  - a new `openapi_contract_test.go` for spec validation and surface assertions
  - a tiny shared test helper if the new contract needs repo-root file loading that should not be duplicated

## Improve-Code-Boundaries Focus

- Primary boundary smell to flatten:
  - verify API contract knowledge is currently split across:
    - handler tests in `verifyservice/http_test.go`
    - README examples in Rust tests
    - no canonical machine-readable contract artifact
- Required cleanup during execution:
  - make `openapi/verify-service.yaml` the single canonical contract artifact for the full verify HTTP surface
  - make one Go-owned contract test file responsible for validating that artifact
  - keep README tests limited to the operator-facing pointer to the canonical spec location
- Bold refactor allowance:
  - if the command package test file becomes too mixed after adding OpenAPI checks, split small reusable helpers out into a dedicated `*_test.go` support file rather than growing `verifyservice_test.go`
  - if a README sentence exists only to duplicate what the OpenAPI file will now own, shorten or remove it

## Intended Public Contract

- Add a canonical spec file at `openapi/verify-service.yaml`.
- Use OpenAPI `3.0.x`.
- Set the default server URL to `http://localhost:8080`.
- Add a description note that the real listener host and port come from `listener.bind_addr`.
- Document exactly these paths and no runner endpoints:
  - `POST /jobs`
  - `GET /jobs/{job_id}`
  - `POST /jobs/{job_id}/stop`
  - `POST /tables/raw`
  - `GET /metrics`
- Include explicit enums where the runtime already exposes stable string values:
  - `JobStatus`:
    - `running`
    - `succeeded`
    - `failed`
    - `stopped`
  - raw table database:
    - `source`
    - `destination`
  - operator error category examples:
    - `request_validation`
    - `job_state`
    - `source_access`
    - `mismatch`
    - `verify_execution`
- Include realistic examples drawn from the existing tests for:
  - job accepted
  - job running
  - job succeeded with result summary
  - job failed with actionable `failure`
  - job stopping
  - raw table read
  - metrics text
  - unknown-field and job-not-found errors
- Document the stateful retention rule:
  - only the most recent completed job is retained
- Model the current error split honestly:
  - structured operator-error envelope for job-related validation/state failures and `413`
  - plain `{ "error": ... }` envelope for current `/tables/raw` validation/disabled responses
- Do not include:
  - runner webhook endpoints
  - internal Go type names
  - mutex/goroutine notes
  - future endpoints
  - auth schemes beyond a brief listener-level TLS note if needed

## Files And Structure To Add Or Change

- `openapi/verify-service.yaml`
  - new canonical OpenAPI document
- `cockroachdb_molt/molt/cmd/verifyservice/openapi_contract_test.go`
  - new default-lane contract test for the spec file
- `cockroachdb_molt/molt/cmd/verifyservice/test_support_test.go`
  - only if needed to share repo-root/spec-loading helpers cleanly
- `cockroachdb_molt/molt/go.mod`
  - add an OpenAPI validation dependency if needed for the contract test
- `cockroachdb_molt/molt/go.sum`
  - dependency lock updates
- `README.md`
  - add one concise pointer to `openapi/verify-service.yaml`
  - trim nearby prose as needed to stay within the word-count contract
- `crates/runner/tests/readme_operator_surface_contract.rs`
  - add one operator-surface assertion for the canonical OpenAPI reference location

## Vertical TDD Slices

### Slice 1: README Pointer To The Canonical Spec

- RED:
  - add a failing README operator-surface contract that requires the verify quick-start to mention `openapi/verify-service.yaml`
  - keep the existing word-count and heading-shape contract intact
- GREEN:
  - add the shortest useful README pointer to the spec location
  - trim nearby wording enough to stay at or below the existing word budget
- REFACTOR:
  - keep README assertions limited to discoverability of the canonical spec, not the full HTTP contract details

### Slice 2: Tracer Bullet For Spec Existence And Validation

- RED:
  - add a failing Go contract test in `./cmd/verifyservice` that:
    - loads `openapi/verify-service.yaml`
    - validates it as OpenAPI 3.0
    - requires the `http://localhost:8080` server entry
    - requires a note that `listener.bind_addr` controls the actual bind address
- GREEN:
  - add the minimal valid OpenAPI document with top-level info, server, and path skeletons
- REFACTOR:
  - extract any repo-path/spec-loading helper needed by the new contract into a small dedicated test helper file

### Slice 3: Job Endpoints, Parameters, And Structured Errors

- RED:
  - tighten the Go spec contract to require:
    - `POST /jobs` request schema with only the four flat filter fields
    - `202` example with `job_id` and `status`
    - `409` structured operator error for already-running jobs
    - `400` structured operator error examples for invalid filter, unknown field, and multiple documents
    - `413` structured operator error for oversized bodies
    - `GET /jobs/{job_id}` path parameter plus running/succeeded/failed/stopped response coverage
    - `404` structured operator error for unknown job
    - `POST /jobs/{job_id}/stop` empty-object request body and `200` stopping example
- GREEN:
  - fill in the job schemas, shared components, and examples
- REFACTOR:
  - consolidate repeated error/result fragments under components instead of duplicating them per operation

### Slice 4: Raw Table And Metrics Contract

- RED:
  - tighten the Go spec contract to require:
    - `POST /tables/raw` request schema with `database`, `schema`, and `table`
    - raw-table database enum values `source` and `destination`
    - `200` raw-table success example with `columns` and `rows`
    - `403` disabled example using the current plain error envelope
    - `400` invalid raw-table example using the current plain error envelope
    - `GET /metrics` `text/plain` response
- GREEN:
  - document the raw-table and metrics surfaces accurately
- REFACTOR:
  - keep the spec explicit about which endpoints return which error envelope so the contract stays honest instead of pretending the API is more uniform than it is

### Slice 5: Stateful Behavior Notes And Final Examples

- RED:
  - tighten the Go contract to require:
    - a note that only the most recent completed job is retained
    - examples for succeeded and failed completed jobs that match the current result/failure shape
    - no runner-only paths or internal type names anywhere in the spec text
- GREEN:
  - finish the examples, descriptions, and exclusions
- REFACTOR:
  - collapse any noisy schema duplication into named components so the file stays readable in Swagger UI and diffs stay reviewable

### Slice 6: Final Lanes And Boundary Pass

- RED:
  - after the contract slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long`
- GREEN:
  - continue until every required default lane passes
- REFACTOR:
  - do one final `improve-code-boundaries` pass and confirm:
    - the README only points to the spec
    - the Go command package owns spec validation
    - the canonical API contract lives in one repo-root OpenAPI file

## TDD Guardrails For Execution

- One failing slice at a time.
- Do not write the whole spec first and then add all assertions afterward.
- Do not add spec coverage in a test lane that `make test` does not run.
- Do not silently normalize `/tables/raw` plain-error responses into operator errors unless a RED slice first proves that contract change is necessary and desirable.
- Do not invent undocumented future fields for job results or metrics.
- Do not document runner endpoints in the verify-service spec.
- If execution discovers that the actual response shape differs materially from this plan, switch this plan back to `TO BE VERIFIED` immediately instead of forcing a misleading spec through.

## Final Verification For The Execution Turn

- [x] `openapi/verify-service.yaml` exists and validates as OpenAPI 3.0
- [x] The spec documents `POST /jobs`, `GET /jobs/{job_id}`, `POST /jobs/{job_id}/stop`, `POST /tables/raw`, and `GET /metrics`
- [x] `POST /jobs` request schema uses only the flat filter fields
- [x] `POST /jobs/{job_id}/stop` request schema is an empty object
- [x] `/tables/raw` request schema documents `database`, `schema`, and `table`
- [x] `job_id` path parameter is documented as a string
- [x] Job status enums are explicitly listed: `running`, `succeeded`, `failed`, `stopped`
- [x] Error category examples are explicitly listed: `request_validation`, `job_state`, `source_access`, `mismatch`, `verify_execution`
- [x] The spec includes realistic request and response examples for all public operations
- [x] The spec models structured operator-error responses where the handlers actually return them
- [x] The spec models plain raw-table error responses where the handlers currently return them
- [x] The spec notes that only the most recent completed job is retained
- [x] The spec does not include runner endpoints or internal Go type names
- [x] `README.md` references `openapi/verify-service.yaml`
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` not run because this is not a story-end task
- [x] Final `improve-code-boundaries` pass confirms the API contract now has one canonical machine-readable owner
- [x] Update the task file and set `<passes>true</passes>` only after all required lanes pass

Plan path: `.ralph/tasks/story-03-docs-api-contracts/task-08-docs-openapi-verify-api_plans/2026-04-25-openapi-verify-api-plan.md`

NOW EXECUTE
