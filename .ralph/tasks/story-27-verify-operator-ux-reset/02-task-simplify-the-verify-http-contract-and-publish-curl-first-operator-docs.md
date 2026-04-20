## Task: Simplify the verify HTTP contract and publish curl-first operator docs <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Redesign the verify HTTP request and response contract so it is obvious to an operator what to send and what comes back, then document the whole flow with copy-pasteable curl examples. The higher order goal is to make the verify service self-explanatory enough that a user can start, poll, and stop a verify job without reading source code or guessing request JSON.

Current product gap from the 2026-04-20 user review:
- there is no user-facing guidance on what to send to the verify HTTP endpoints
- the request shape is too deeply nested and not “one simple top-level thing”
- fields such as `enabled` are buried in inner structs instead of using a simpler contract
- the public operator docs do not show a complete curl-driven flow

In scope:
- simplify `POST` start-job request JSON so the live inputs are as flat and direct as possible while preserving the config-owned connection boundary
- simplify any stop or result-query request/response shapes that currently require needless nesting
- remove or flatten inner `enabled` structs where a direct field, omitted field, or separate endpoint is clearer
- document the verify HTTP API in the README or another explicit operator-facing doc that is kept under test
- include curl examples for at least: starting a job, polling a job, reading the final result, and stopping a running job
- include example responses for success, failure, and validation error cases
- make the request and response shape explicit in tests so the docs cannot drift away from the real contract

Out of scope:
- changing database connection ownership so HTTP callers can supply URLs or TLS material
- implementing richer mismatch analysis beyond what the result contract already exposes

Decisions already made:
- operator docs must explain the verify HTTP surface directly instead of forcing users to infer it from tests or source code
- the verify HTTP contract should stay narrow, but “narrow” must not mean obscure
- request shapes should be flattened aggressively when nesting provides no real value
- curl examples must be copy-pasteable and must reflect the supported contract exactly
- this task should update the registry-only novice-user documentation and tests when the verify examples change

Relevant files and boundaries:
- `cockroachdb_molt/molt/verifyservice/http_test.go`
- `cockroachdb_molt/molt/verifyservice/job.go`
- `cockroachdb_molt/molt/verifyservice/service.go`
- `cockroachdb_molt/molt/verifyservice/result.go`
- `README.md`
- `crates/runner/tests/readme_operator_surface_contract.rs`
- `crates/runner/tests/novice_registry_only_contract.rs`
- `crates/runner/tests/support/novice_registry_only_harness.rs`
- `crates/runner/tests/support/readme_operator_surface.rs`

</description>


<acceptance_criteria>
- [x] Red/green TDD proves the verify HTTP start, poll, result, and stop contracts use the simplified request/response shapes
- [x] The start-job request body is flattened so operators do not have to navigate needless nested `enabled` wrappers or equivalent extra structure
- [x] The README or dedicated operator-facing doc includes copy-pasteable curl examples for start, poll, final result, and stop flows
- [x] Example success, failure, and validation-error JSON responses are documented and covered by tests
- [x] Registry-only novice-user verification coverage is updated so the verify HTTP docs are proven from the published-image operator path
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-27-verify-operator-ux-reset/02-task-simplify-the-verify-http-contract-and-publish-curl-first-operator-docs_plans/2026-04-20-verify-http-contract-and-curl-first-docs-plan.md</plan>
