# Plan: Document Runner Webhook Payload Format For API Consumers

## References

- Task:
  - `.ralph/tasks/story-03-docs-api-contracts/task-07-docs-webhook-payload-format.md`
- Current operator-facing docs:
  - `README.md`
- Runner webhook payload and HTTP contract:
  - `crates/runner/src/webhook_runtime/payload.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/error.rs`
  - `crates/runner/tests/webhook_contract.rs`
- README operator-surface contract:
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/support/readme_operator_surface.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the public interface direction in this planning turn.
- This turn is planning-first because task 07 had no existing plan artifact.
- The public contract to change is documentation plus its README contract tests, not the runtime payload parser shape.
- The current ingest wire contract is already implemented and should be documented honestly:
  - request shapes are row-batch or resolved
  - response codes are `200`, `400`, `404`, and `500`
  - error bodies currently return plain-text messages, not structured JSON
- The existing README operator-surface contract is tight:
  - second-level headings must remain only `Setup SQL Quick Start`, `Runner Quick Start`, and `Verify Quick Start`
  - total README word count must stay at or below `1250`
- Current README word count is `1243`, so execution must remove or compress muddy prose while adding the webhook payload section.
- The new payload docs should live under the existing runner section as a third-level heading:
  - `### Webhook Payload Format`
  - this satisfies the task requirement without breaking the current top-level heading contract
- If the first RED slice proves that a usable payload section cannot fit inside the README word budget without making the quick start muddy, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - README contains a dedicated `Webhook Payload Format` subsection under `Runner Quick Start`
  - README shows one complete row-batch JSON example with at least two rows:
    - one upsert event
    - one delete event
  - README shows one complete resolved JSON example
  - README explains top-level fields and nested fields with a field-by-field table
  - README documents valid `op` values and their meanings:
    - `c`
    - `u`
    - `r`
    - `d`
  - README explains that all rows in one batch must belong to the same source table
  - README explains that `length` must equal `payload` array length
  - README includes a copy-pasteable `curl` example for `POST /ingest/app-a`
  - README documents `200 OK`, `400 Bad Request`, `404 Unknown Mapping`, and `500 Internal Server Error`
  - README does not mention internal Rust type names or source file paths
- Lower-priority concerns:
  - keeping every current runner prose sentence if some of it must be shortened to preserve the quick-start budget
  - adding new runtime tests for behavior that is already implemented and would only produce "just GREEN" coverage

## Current State Summary

- The README currently lists `POST /ingest/<mapping_id>` but does not describe the request body at all.
- The payload parser accepts exactly two supported shapes:
  - row-batch requests with `length` and `payload`
  - resolved requests with non-empty `resolved`
- Row-batch events currently require:
  - `source.database_name`
  - `source.schema_name`
  - `source.table_name`
  - `op`
  - `key`
  - `after` for upsert-style events
- Valid `op` values already map to the public behavior we need to document:
  - `c`, `u`, and `r` are accepted as upsert-style events
  - `d` is accepted as delete
- Routing enforces two important invariants that belong in the docs:
  - all rows in one batch must target the same mapped source table
  - the source database/table must match the mapping selected by `/ingest/<mapping_id>`
- HTTP response handling already exposes the public status split we need to document:
  - `200` for accepted row-batch and resolved requests
  - `400` for payload-shape and routing contract violations
  - `404` for unknown mapping ids
  - `500` for persistence failures
- Error response bodies are currently plain text produced from the public error messages.
- The README is already near the word-budget ceiling, so execution cannot just append a long new section without deleting or compressing existing runner prose.

## Boundary Decision

- Make the README operator-surface contract the single owner of the new payload-documentation shape.
- Do not spread webhook payload documentation assertions across unrelated runtime tests.
- Keep runtime tests focused on runtime behavior and use them only as the factual source for the docs contract.
- Improve one real boundary smell while doing the docs task:
  - current README tests only expose second-level section extraction
  - execution should add a small helper in `readme_operator_surface.rs` for extracting a subsection from within `Runner Quick Start`
  - that keeps markdown-structure parsing in the helper layer instead of repeated raw `find()` / `contains()` slicing inside the contract test

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - README payload contract knowledge would otherwise be split between:
    - raw markdown searches in tests
    - runtime source inspection
    - ad hoc README prose with no contract owner
- Required cleanup during execution:
  - make `readme_operator_surface.rs` own subsection extraction for `### Webhook Payload Format`
  - make `readme_operator_surface_contract.rs` own the README payload surface assertions
  - keep `webhook_contract.rs` unchanged unless execution discovers a genuinely missing public-behavior assertion that first fails RED
- Bold refactor allowance:
  - if existing runner README prose is the only thing preventing the new section from fitting inside the word budget, delete or merge that prose instead of trying to preserve every sentence

## Intended Public Contract

- Under `## Runner Quick Start`, add `### Webhook Payload Format`.
- The subsection should teach only the supported operator payload surface:
  - one row-batch example
  - one resolved example
  - one field table
  - one short `op` value list
  - one short invariants list
  - one `curl` example
  - one short response-code list or table
- The row-batch example must be complete and realistic:
  - `length: 2`
  - one upsert row
  - one delete row
  - both rows from the same `source` table
  - `key` and `after` represented as arbitrary JSON column maps
- The resolved example must be visibly distinct from the row-batch shape:
  - only the `resolved` watermark field
- The response-code documentation must match the existing wire contract:
  - `200 OK`
  - `400 Bad Request` with a plain-text example body
  - `404 Unknown Mapping`
  - `500 Internal Server Error`
- The subsection must avoid:
  - internal type names
  - source file paths
  - implementation details about persistence, SQL generation, routing internals, metrics, auth, or TLS

## Files And Structure To Add Or Change

- `README.md`
  - add `### Webhook Payload Format` under `## Runner Quick Start`
  - add the required examples, field table, invariants, curl example, and response codes
  - compress or remove nearby runner prose as needed to stay within the README word budget
- `crates/runner/tests/support/readme_operator_surface.rs`
  - add a small helper for extracting a named subsection within a top-level README section
- `crates/runner/tests/readme_operator_surface_contract.rs`
  - add or tighten contract coverage for the webhook payload subsection
  - keep the existing second-level heading and word-count contract intact

## Vertical TDD Slices

### Slice 1: Tracer Bullet For The New README Subsection

- RED:
  - add a failing README operator-surface contract that requires:
    - `### Webhook Payload Format` inside `## Runner Quick Start`
    - unchanged second-level heading structure
    - README word count still at or below `1250`
- GREEN:
  - add the minimal subsection heading and trim existing runner prose enough to keep the document inside the word budget
- REFACTOR:
  - add the README subsection helper so the contract stops depending on raw string slicing

### Slice 2: Row-Batch Shape And Invariants

- RED:
  - tighten the README contract to require:
    - a complete row-batch example with `length: 2`
    - one upsert event and one delete event
    - `source.database_name`, `schema_name`, and `table_name`
    - `key`
    - `after`
    - the valid `op` meanings list
    - explicit notes that all rows in a batch must belong to the same source table
    - explicit note that `length` must match `payload` length
- GREEN:
  - write the concise row-batch example, field table, and invariants list in the README
- REFACTOR:
  - remove repetitive prose in the surrounding runner section so the new contract remains readable and short

### Slice 3: Resolved Shape, Curl Example, And Response Codes

- RED:
  - tighten the README contract to require:
    - a complete resolved example
    - a copy-pasteable `curl` example that targets `/ingest/app-a`
    - `200`, `400`, `404`, and `500` response-code documentation
    - one plain-text `400` example body that matches the real error style
    - absence of internal Rust type names and source-file references
- GREEN:
  - fill in the remaining subsection content with concise examples and response-code documentation
- REFACTOR:
  - collapse any verbose response prose into compact bullets or a table so the quick-start still reads like an operator guide

### Slice 4: Final Lanes And Boundary Pass

- RED:
  - after the README contract slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long`
- GREEN:
  - continue until every required default lane passes
- REFACTOR:
  - do one final `improve-code-boundaries` pass and confirm the payload-doc contract now has one clear owner in the README test/helper layer

## TDD Guardrails For Execution

- One failing slice at a time.
- Do not bulk-write all README assertions first.
- Test through public surfaces only:
  - `README.md`
  - `ReadmeOperatorSurface`
  - existing README operator-surface contract
- Do not invent a different error-body shape than the runtime currently returns.
- Do not add new runtime tests that are already green without a prior failing slice.
- Do not add a new second-level heading for the payload docs.
- If execution discovers the payload examples cannot stay both accurate and short inside the README quick-start boundary, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Final Verification For The Execution Turn

- [ ] README contains `### Webhook Payload Format` under `## Runner Quick Start`
- [ ] Row-batch example is complete and includes at least two rows
- [ ] Row-batch docs explain valid `op` values: `c`, `u`, `r`, `d`
- [ ] Resolved example is complete and distinct from row-batch
- [ ] Field description table covers top-level and nested fields
- [ ] Docs explain `source.database_name`, `source.schema_name`, and `source.table_name`
- [ ] Docs explain `key` and `after` as arbitrary JSON column maps
- [ ] Docs explain that all rows in a batch must belong to the same source table
- [ ] Docs explain that `length` must match `payload` array length
- [ ] `curl` example targets `/ingest/app-a` and is copy-pasteable
- [ ] Response codes document `200`, `400`, `404`, and `500`
- [ ] README contains no internal Rust type names or source-file references for webhook payload docs
- [ ] README operator surface contract passes
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long` not run because this is not a story-end task
- [ ] Final `improve-code-boundaries` pass confirms the README payload contract is cleaner than before
- [ ] Update the task file and set `<passes>true</passes>` only after all required lanes pass

Plan path: `.ralph/tasks/story-03-docs-api-contracts/task-07-docs-webhook-payload-format_plans/2026-04-25-webhook-payload-format-plan.md`

NOW EXECUTE
