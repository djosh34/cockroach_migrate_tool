# Plan: Return Full Verify Job Findings, Mismatches, And Human-Usable Result JSON

## References

- Task:
  - `.ralph/tasks/story-27-verify-operator-ux-reset/04-task-return-full-verify-job-findings-mismatches-and-human-usable-result-json.md`
- Current Go verify-service result boundary:
  - `cockroachdb_molt/molt/verifyservice/result.go`
  - `cockroachdb_molt/molt/verifyservice/result_test.go`
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/error.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- Current typed verify event stream:
  - `cockroachdb_molt/molt/verify/inconsistency/reporter.go`
  - `cockroachdb_molt/molt/verify/inconsistency/row.go`
  - `cockroachdb_molt/molt/verify/inconsistency/table.go`
- Current Rust verify-image contract:
  - `crates/runner/tests/verify_job_result_contract.rs`
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
- Operator docs:
  - `README.md`
- Skill:
  - `tdd`
- Skill:
  - `improve-code-boundaries`

## Planning Assumptions

- This is an execution plan only for the verify job result contract.
  - no arbitrary SQL endpoints
  - no log parsing as a supported correctness surface
  - no fake or invented mismatch data
- `GET /jobs/{job_id}` must become the supported operator truth source for:
  - final correctness
  - mismatch diagnosis
  - infrastructure-failure diagnosis
- The current lifecycle states remain unchanged:
  - `running`
  - `succeeded`
  - `failed`
  - `stopped`
- The current top-level `failure` field remains the lifecycle-level failure classifier.
  - mismatch failures stay distinct from source-access, destination-access, and verify-execution failures
  - the richer `result` payload explains what happened
- The task must deepen the existing result contract from story 18 rather than layering a second competing shape on top.
- If the first RED slice proves the real verify stream does not expose enough typed data to build a stable operator contract without scraping logs or parsing error strings, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.
- If the first RED slice proves the chosen JSON shape duplicates the same finding across multiple transport DTOs or requires stringly post-processing in Rust, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `verifyservice/result.go` currently retains only three coarse buckets:
  - `table_summaries`
  - `mismatch_tables`
  - `table_definition_mismatches`
- That means the service loses the most operator-useful facts from typed verify events:
  - row primary keys
  - missing-row versus extraneous-row distinction
  - mismatching-column names
  - source-versus-destination value evidence already available on mismatch events
- `result.go` also constructs mismatch failure details separately from the retained result data.
  - the operator sees `"mismatch detected for schema.table"` instead of the actual findings that led to the mismatch
- `verify_runner.go` and `error.go` classify infrastructure failures cleanly, but the final HTTP result does not pair those failures with any partial findings emitted before the failure.
- The Rust harness and contract tests already consume structured job JSON, but only for the shallow story-18 result shape.
  - they cannot yet assert richer mismatch details or partial result context
- README examples still need a success payload and a mismatch payload that are actually useful for operators.

## Improve-Code-Boundaries Focus

- Primary smell: wrong-placeism.
  - typed mismatch evidence exists in `inconsistency` events, but the service collapses it into table-name lists and a generic message
  - that pushes real diagnosis back into logs, which is the wrong boundary
- Secondary smell: mixed responsibilities.
  - `result.go` both stores mutable aggregation state and defines a shallow transport view that is no longer rich enough for the product goal
- Boundary goal:
  - one canonical internal verify-result aggregate in `verifyservice`
  - one transport rendering of that aggregate for `GET /jobs/{job_id}`
  - no separate ad hoc mismatch-message synthesis that hides the actual findings
- Preferred refactor direction:
  - replace multiple parallel mismatch DTO buckets with a canonical finding model plus per-table summaries derived from the same source of truth
  - keep `failure` classification typed and separate from correctness findings, but let the final response show both together

## Public Contract Decisions

### Final Job JSON

- Keep `GET /jobs/{job_id}` as the only supported result endpoint.
- Preserve the existing top-level fields:
  - `job_id`
  - `status`
  - optional `failure`
  - optional `result`
- Deepen `result` so it explains both success and failure in structured JSON.
- Preferred response shape:
  - `result.summary`
    - aggregate counts for verified tables and mismatch categories
  - `result.table_summaries[]`
    - per-table rolled-up counts, preserving the current fields already used by the Rust harness
  - `result.findings[]`
    - canonical ordered list of detailed findings
  - `result.mismatch_summary`
    - structured mismatch totals and affected tables for fast operator scanning
- `result.findings[]` should use a typed schema rather than free-form messages.
  - required common fields:
    - `kind`
    - `schema`
    - `table`
  - kind-specific fields should carry real evidence from the verify event stream, for example:
    - missing row:
      - `primary_key`
    - extraneous row:
      - `primary_key`
    - mismatching row or column:
      - `primary_key`
      - `mismatching_columns`
      - `source_values`
      - `destination_values`
      - optional `info`
    - table definition mismatch:
      - `message`
    - missing or extraneous table:
      - no row payload required beyond table identity
- `result.mismatch_summary` should make the operator path obvious without parsing `findings[]`.
  - preferred fields:
    - `has_mismatches`
    - `affected_tables`
    - `counts_by_kind`
- For successful runs:
  - `failure` is omitted
  - `result.summary` and `result.table_summaries[]` still show what was verified
  - `result.findings[]` is empty
- For mismatch-driven failures:
  - top-level `failure.category` stays `mismatch`
  - `result.findings[]` and `result.mismatch_summary` explain exactly what mismatched
- For infrastructure or execution failures:
  - top-level `failure.category` remains `source_access`, `destination_access`, or `verify_execution`
  - `result` should still be present if any table summaries or findings were emitted before the failure
  - the response must not misclassify those failures as mismatches just because some findings already existed

### Datum Rendering Rules

- Use machine-readable structure first, human-readable strings second.
- Primary-key values and row values should be rendered from the actual typed datums the verify stream already carries.
- Reuse one explicit formatter helper for result JSON instead of duplicating log-only formatting logic.
- Do not silently drop unsupported values.
  - if a chosen rendering strategy cannot represent a value honestly, execution must switch back to `TO BE VERIFIED`

## Proposed Code Shape

- `cockroachdb_molt/molt/verifyservice/result.go`
  - own the canonical aggregate and the canonical finding type
  - aggregate all supported typed mismatch and summary events
  - render the final result DTO from that aggregate
- `cockroachdb_molt/molt/verifyservice/result_test.go`
  - drive the aggregate through reportable events directly
  - assert per-kind findings, mismatch summaries, and partial-result behavior
- `cockroachdb_molt/molt/verifyservice/service.go`
  - keep job lifecycle ownership
  - stop building mismatch-only failure details from a table list when richer result findings exist
  - on completion, pair lifecycle failure classification with the richer retained result
- `cockroachdb_molt/molt/verifyservice/http_test.go`
  - assert full HTTP payloads for:
    - successful verification
    - mismatch-driven failure
    - infrastructure or execution failure after partial progress
- `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
  - keep infrastructure-failure classification coverage
  - add coverage only if a richer structured failure detail contract is introduced here
- `crates/runner/tests/support/e2e_integrity.rs`
  - extend `VerifyJobResponse` and `VerifyCorrectnessAudit` to decode and reason about the richer result contract
  - keep the audit focused on public behavior, not Go implementation details
- `crates/runner/tests/support/verify_image_harness.rs`
  - continue using the HTTP payload as the only correctness source
  - preserve container logs only for diagnostic panic output
- `crates/runner/tests/verify_job_result_contract.rs`
  - add public-contract deserialization and behavior assertions for richer success and mismatch payloads
- `README.md`
  - add one useful success payload example
  - add one useful mismatch payload example
  - show concrete fields operators can rely on

## Internal Type Decisions

- Introduce one canonical finding type owned by `verifyservice`, for example:
  - `jobFinding`
- Keep one internal owner for table identity, reused across:
  - per-table summaries
  - mismatch summary
  - detailed findings
- Collapse duplicate mismatch bookkeeping where possible.
  - if `mismatch_tables` can be derived from canonical findings plus table-definition mismatches, remove the redundant mutable map instead of keeping parallel sources of truth
- Keep transport DTOs separate from mutable aggregation state.
  - aggregate types stay internal
  - response DTOs stay read-only render views

## TDD Slices

### Slice 1: Tracer Bullet For Rich Success Results

- [ ] RED: add one failing Go HTTP test proving `GET /jobs/{job_id}` returns a useful success payload with summary data and an explicit empty findings list
- [ ] GREEN: add the smallest result-summary shape and renderer needed to make that test pass
- [ ] REFACTOR: keep the aggregate type separate from the JSON DTO so later finding detail does not leak transport concerns back into storage

### Slice 2: Detailed Mismatch Findings Through HTTP

- [ ] RED: add one failing Go HTTP test proving a mismatch job returns structured finding details for at least one real mismatch event type instead of only table-name lists
- [ ] GREEN: retain and render detailed mismatch evidence from typed `inconsistency` events
- [ ] REFACTOR: collapse mismatch bookkeeping so the response does not depend on parallel mutable maps plus separately synthesized messages

### Slice 3: Distinguish Mismatch Failures From Infrastructure Failures

- [ ] RED: add one failing Go HTTP test proving infrastructure failure returns the correct top-level failure category while preserving any partial result already emitted
- [ ] GREEN: pair `classifyRunFailure(err)` with the retained canonical result without reclassifying the job as a mismatch
- [ ] REFACTOR: keep failure classification in one place and stop duplicating error-bucket decisions across service and result code

### Slice 4: Result Aggregate Unit Coverage

- [ ] RED: add one failing Go unit test in `result_test.go` for at least one row-level mismatch finding that must carry primary-key and source-versus-destination detail
- [ ] GREEN: add the minimal canonical finding projection needed to satisfy that behavior
- [ ] REFACTOR: extract one reusable datum-rendering helper so row evidence is formatted once for both summary and detailed finding paths

### Slice 5: Rust Contract And Harness Alignment

- [ ] RED: add one failing Rust contract test proving the richer HTTP result payload deserializes and drives the correctness audit without log scraping
- [ ] GREEN: extend the Rust response types and audit helpers to consume the richer contract
- [ ] REFACTOR: keep Rust assertions focused on the public result contract, not specific Go field-order or internal aggregation details

### Slice 6: Operator Docs

- [ ] RED: update README examples and, if needed, doc assertions so the new success and mismatch payloads are documented explicitly
- [ ] GREEN: document one useful success payload and one useful mismatch payload with the fields operators should inspect first
- [ ] REFACTOR: remove or rewrite any stale docs that still imply the result is basically a coarse `error` field

## TDD Guardrails For Execution

- One failing behavioral slice at a time.
- Do not add tests after implementation for the same behavior.
- Test through public contracts first:
  - Go HTTP job JSON
  - Rust HTTP-result decoding and audit behavior
- Do not hide mismatch evidence in stringified logs or synthesized summary-only messages.
- Do not swallow any errors while projecting typed findings into JSON.
- If detailed mismatch rendering requires lossy conversion or ad hoc parsing of log strings, switch the plan back to `TO BE VERIFIED` and stop immediately.

## Final Verification For The Execution Turn

- [ ] Red/green TDD covers success results, mismatch results, and infrastructure-failure results through the HTTP job JSON contract
- [ ] Final job JSON includes structured findings that explain what was wrong rather than only a coarse `error` field
- [ ] Mismatch-driven failures are clearly separated from transport/auth/config/process failures
- [ ] The Rust verify-image harness and result-contract tests assert the richer HTTP result payload directly rather than depending on log scraping for core correctness details
- [ ] Docs and curl examples show at least one useful mismatch result payload and one useful success result payload
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] Do not run `make test-long` unless the task explicitly requires it or this work changes the long-lane selection
- [ ] Final `improve-code-boundaries` pass confirms the verify result boundary got smaller and more honest
- [ ] Update the task file checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-27-verify-operator-ux-reset/04-task-return-full-verify-job-findings-mismatches-and-human-usable-result-json_plans/2026-04-20-verify-job-full-findings-result-plan.md`

NOW EXECUTE
