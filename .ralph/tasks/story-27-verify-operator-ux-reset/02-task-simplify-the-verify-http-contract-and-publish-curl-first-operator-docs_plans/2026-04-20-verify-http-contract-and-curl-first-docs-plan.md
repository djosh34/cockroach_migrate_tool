# Plan: Simplify Verify HTTP Contract And Publish Curl-First Operator Docs

## References

- Task:
  - `.ralph/tasks/story-27-verify-operator-ux-reset/02-task-simplify-the-verify-http-contract-and-publish-curl-first-operator-docs.md`
- Current Go verify HTTP boundary:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/filter.go`
  - `cockroachdb_molt/molt/verifyservice/job.go`
  - `cockroachdb_molt/molt/verifyservice/result.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
- Current operator docs and Rust contract coverage:
  - `README.md`
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/runner/tests/support/readme_operator_surface.rs`
  - `crates/runner/tests/support/novice_registry_only_harness.rs`
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient product approval for this planning turn.
- This turn is planning-only because task 02 had no plan artifact or execute marker yet.
- The existing verify HTTP surface is still shaped around internal DTO nesting instead of operator actions:
  - start requests use `filters.include` and `filters.exclude` wrappers even though the runtime only needs four direct filter strings
  - stop requests accept an optional `job_id` body and return `stopped_job_ids`, which is misleading for a single-active-job service
  - README documents how to start the verify process, but not how to drive the HTTP API after startup
- Rust-side consumers currently depend on:
  - `job_id` from the start response
  - `GET /jobs/{job_id}` final result JSON
  - they do not currently depend on the nested request DTO shape
- This is greenfield work with no backwards-compatibility requirement.
  - remove awkward HTTP shapes instead of translating them forward
  - delete obsolete docs/tests/fixtures once replacement coverage exists
- If the first RED slice shows the job-stop path should stay body-driven rather than path-driven, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - start-job accepts a flat, direct request body through the public HTTP API
  - poll and final result responses are explicit enough to document without reading source
  - stop flow is job-scoped and does not expose fake multi-job list semantics
  - README examples are copy-pasteable curl sequences for start, poll, final result, and stop
  - README examples for success, failure, and validation-error JSON stay under test
  - registry-only novice-user coverage keeps proving the published-image operator path
- Lower-priority concerns:
  - richer runtime failure detail beyond what this task needs to document cleanly
  - preserving any old nested request/response shape for migration purposes

## Current State Summary

- Start-job request shape is unnecessarily nested:
  - `JobRequest` only carries `filters.include.schema`, `filters.include.table`, `filters.exclude.schema`, and `filters.exclude.table`
  - `RunRequest` ultimately compiles those four strings into `utils.FilterConfig`
  - the `filters/include/exclude` wrappers do not represent a real runtime boundary
- Stop flow leaks the wrong abstraction:
  - `POST /stop` accepts either an empty body or a `job_id`
  - the service only supports one active job
  - the response still returns `stopped_job_ids []string`, which implies bulk semantics that do not exist
- Poll/result flow is usable but under-documented:
  - `GET /jobs/{job_id}` already exposes the public job lifecycle and structured success result
  - failure and validation examples are not documented as operator-facing JSON
- README does not yet explain the verify HTTP surface:
  - it documents config and container startup only
  - no curl examples show how to start a job, poll it, inspect results, or stop it
- Rust-side operator coverage currently materializes the verify image and polls the job result, but not through README-owned curl examples.

## Boundary Decision

- Flatten the HTTP contract around operator actions instead of internal filter DTOs.
- Preferred start request contract:
  - `include_schema`
  - `include_table`
  - `exclude_schema`
  - `exclude_table`
- Meaning of that contract:
  - omitted `include_*` fields default to the current “match everything” behavior
  - omitted `exclude_*` fields mean no exclusions
  - the config file continues to own source/destination connections and TLS material; HTTP callers only control the live selection inputs
- Preferred stop contract:
  - replace `POST /stop` with `POST /jobs/{job_id}/stop`
  - return one job-scoped response instead of `stopped_job_ids`
- Preferred job response contract:
  - keep `GET /jobs/{job_id}` as the single poll and final-result endpoint
  - keep `job_id` and `status` stable unless the first RED slice proves a stronger rename is justified
  - keep structured success results under `result`
  - document current failed/stopped terminal responses explicitly and only expand them if RED proves the existing shape is too obscure for operator docs

## Improve-Code-Boundaries Focus

- Primary boundary smell to flatten:
  - public HTTP JSON currently mirrors internal `JobFilters`/`NameFilters` DTO nesting instead of the real live-input boundary
- Required cleanup during execution:
  - collapse or delete `JobFilters` and `NameFilters` if the flat request contract lands
  - move request compilation to one small boundary that converts direct HTTP fields into `utils.FilterConfig`
  - delete the bulk-stop response shape if the service remains single-job only
  - keep Rust harness parsing aligned with the minimal public JSON that operators actually see
- Bold refactor allowance:
  - if `JobRequest`, `JobFilters`, `NameFilters`, or the anonymous stop DTO become pass-through shells after flattening, delete the types and update tests instead of preserving compatibility code

## Intended Public Contract

- Start a job:
  - `POST /jobs`
  - request body is one flat JSON object with direct filter fields only
  - success response returns the accepted job identifier and running status
  - validation errors return a direct JSON error payload with no hidden fallback behavior
- Poll a job:
  - `GET /jobs/{job_id}`
  - running response shows the job id and status
  - succeeded response includes the structured verify result
  - failed and stopped responses stay explicit and documentable
- Stop a job:
  - `POST /jobs/{job_id}/stop`
  - success response is job-scoped and does not imply multiple jobs can be stopped at once
  - unknown job id returns a documented `404` JSON error
- Docs:
  - README verify quick start must include copy-pasteable curl examples for:
    - starting a job
    - polling a running job
    - reading a completed job result
    - stopping a running job
  - README must include example JSON for:
    - success
    - failure
    - validation error

## Files And Structure To Add Or Change

- `cockroachdb_molt/molt/verifyservice/filter.go`
  - flatten the job request shape and keep request compilation at one direct boundary
- `cockroachdb_molt/molt/verifyservice/service.go`
  - route the job-scoped stop endpoint
  - return the simplified stop response shape
- `cockroachdb_molt/molt/verifyservice/job.go`
  - keep job response rendering coherent with the documented contract
- `cockroachdb_molt/molt/verifyservice/result.go`
  - preserve structured success results unless a RED slice proves they need a naming cleanup
- `cockroachdb_molt/molt/verifyservice/http_test.go`
  - rewrite request/stop/docs-driving contract tests around the simplified public surface
- `README.md`
  - extend Verify Quick Start with curl-driven API flow and example JSON
- `crates/runner/tests/readme_operator_surface_contract.rs`
  - assert README keeps the new verify curl flow inline and copyable
- `crates/runner/tests/novice_registry_only_contract.rs`
  - keep the registry-only verify path aligned with the published operator docs
- `crates/runner/tests/support/readme_operator_surface.rs`
  - parse or materialize any new inline curl/docs artifacts needed by README tests
- `crates/runner/tests/support/novice_registry_only_harness.rs`
  - execute the README-owned verify flow when the public docs change
- `crates/runner/tests/support/verify_image_harness.rs`
  - send the new flat start request shape
  - adapt to the simplified stop contract if used by coverage
- `crates/runner/tests/support/e2e_integrity.rs`
  - keep final result deserialization aligned if result payload naming changes

## Vertical TDD Slices

### Slice 1: Tracer Bullet For A Flat Start Request

- RED:
  - add one failing Go HTTP test that starts a job with direct top-level filter fields such as `include_schema` and `include_table`
  - prove the runner receives the expected compiled `utils.FilterConfig`
- GREEN:
  - implement the minimum request-shape change needed to accept the flat payload
- REFACTOR:
  - delete obsolete nested request DTOs if they are no longer real boundaries

### Slice 2: Reject The Old Nested And Unknown Request Shapes

- RED:
  - add the next failing tests that reject the old `filters.include/exclude` contract and any connection-like top-level fields
  - keep the validation through the real HTTP handler with unknown-field checks enabled
- GREEN:
  - remove compatibility support for the nested request shape
- REFACTOR:
  - keep one direct compile/validate path for public request fields

### Slice 3: Make Stop Job-Scoped Instead Of List-Shaped

- RED:
  - add one failing test for `POST /jobs/{job_id}/stop`
  - assert the success response is job-scoped rather than `stopped_job_ids`
  - assert unknown job ids still produce a `404` JSON error
- GREEN:
  - implement the smallest route and response change needed for the single-job model
- REFACTOR:
  - delete the old stop-body DTO and bulk-stop response shape if they become dead code

### Slice 4: Preserve Poll And Final Result As The Readable Operator Surface

- RED:
  - add or adjust tests that pin the documented running, succeeded, failed, and stopped response shapes through `GET /jobs/{job_id}`
  - only introduce new failure fields if RED proves the current terminal shape is too thin for operator docs
- GREEN:
  - keep the poll/result response coherent with the public docs
- REFACTOR:
  - centralize job response rendering so README examples, Go tests, and Rust parsing agree on one vocabulary

### Slice 5: Drive The README With Curl-First Operator Docs

- RED:
  - add the next failing README/operator-surface assertions for:
    - curl start command
    - curl poll command
    - curl final-result command
    - curl stop command
    - success, failure, and validation-error example JSON
- GREEN:
  - update README verify quick start with the documented API flow
- REFACTOR:
  - keep README parsing/materialization logic inside the existing Rust support helpers instead of scattering ad hoc string checks

### Slice 6: Re-Prove The Published-Image Operator Path

- RED:
  - add the next failing novice/verify-image harness assertions that use the README-owned start contract and keep the public result contract in sync
- GREEN:
  - update the Rust harnesses to drive the new flat request shape and any stop-path changes
- REFACTOR:
  - remove stale helper names or DTOs that still mention the nested filter contract

### Slice 7: Final Lanes And Boundary Pass

- RED:
  - after the behavior slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long` unless execution ends up changing long-lane selection or the task explicitly requires it
- GREEN:
  - continue until all required default lanes pass cleanly
- REFACTOR:
  - do one final `improve-code-boundaries` pass so the public HTTP JSON, the Go compile boundary, and the README-owned operator contract each live in one honest place

## TDD Guardrails For Execution

- One failing behavioral slice at a time.
- Do not add tests after implementation for the same behavior.
- Do not preserve the old nested request DTO behind hidden compatibility shims.
- Do not keep a fake multi-job stop response in a single-active-job service.
- Do not let HTTP callers supply config-owned connection or TLS inputs.
- Do not swallow validation or runtime errors.
- If the first RED slice shows the chosen public contract is wrong, switch this plan back to `TO BE VERIFIED` and stop immediately instead of forcing the wrong surface through.

## Final Verification For The Execution Turn

- [ ] Red/green TDD proves the verify HTTP start, poll, result, and stop contracts use the simplified request/response shapes
- [ ] The start-job request body is flattened so operators do not have to navigate needless nested `enabled` wrappers or equivalent extra structure
- [ ] The README or dedicated operator-facing doc includes copy-pasteable curl examples for start, poll, final result, and stop flows
- [ ] Example success, failure, and validation-error JSON responses are documented and covered by tests
- [ ] Registry-only novice-user verification coverage is updated so the verify HTTP docs are proven from the published-image operator path
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] Do not run `make test-long` unless the task explicitly requires it or long-lane selection changes

NOW EXECUTE
