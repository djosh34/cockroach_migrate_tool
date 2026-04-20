# Plan: Complete The Verify HTTP JSON Read Surface With Structured Results And Config-Gated Raw Table Output

## References

- Task:
  - `.ralph/tasks/story-18-verify-http-image/11-task-add-config-gated-raw-source-and-destination-table-json-output-to-verify-http.md`
- Current verify-service HTTP boundary:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- Current verify report stream and typed findings:
  - `cockroachdb_molt/molt/verify/inconsistency/reporter.go`
  - `cockroachdb_molt/molt/verify/inconsistency/stats.go`
  - `cockroachdb_molt/molt/verify/inconsistency/table.go`
  - `cockroachdb_molt/molt/verify/inconsistency/row.go`
- Current DB connection boundary:
  - `cockroachdb_molt/molt/dbconn/dbconn.go`
  - `cockroachdb_molt/molt/dbconn/pg.go`
- Current Rust verify-image correctness boundary:
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
- Skill:
  - `tdd`
- Skill:
  - `improve-code-boundaries`

## Planning Assumptions

- This task owns one product boundary, not two:
  - the verify-service JSON read surface
  - both machine-readable job results and operator raw-table inspection belong to that same HTTP boundary
- `GET /jobs/{job_id}` must become the correctness contract again.
  - Rust test support must stop reconstructing correctness from verify container logs
  - logs may remain diagnostic output, but not the asserted API
- The current lifecycle states stay unchanged:
  - `running`
  - `succeeded`
  - `failed`
  - `stopped`
- Raw-table inspection must fail closed by config.
  - disabled by default
  - no HTTP caller-supplied DB URL, TLS, verify mode, or SQL fragment
- The raw-table feature must stay narrow.
  - only schema/table selection plus which side to inspect
  - no arbitrary SQL
  - no caller-controlled connection material
- If the first red slice proves the current typed verify event stream cannot honestly express the selected-table correctness contract without falling back to log scraping or inventing semantics, execution must switch this plan back to `TO BE VERIFIED` immediately.
- If the first raw-table slice proves the retained `dbconn` boundary cannot produce JSON-safe row values without silent field dropping, execution must switch this plan back to `TO BE VERIFIED` immediately instead of sneaking in lossy behavior.

## Current State Summary

- `verifyservice.Service` currently stores only coarse job lifecycle state.
  - `job.response()` returns `{job_id,status}`
  - `recordReport` is a no-op sink
- The service already observes the right typed source of truth.
  - `VerifyRunner.Run(...)` passes a live `inconsistency.Reporter` into `verify.Verify(...)`
  - summary and mismatch events already cross the service boundary as typed Go values
- The current correctness boundary is split across layers.
  - Go service owns execution and job lifecycle
  - Rust harness derives correctness by parsing `docker logs`
  - that is classic wrong-placeism and mixed responsibility
- `crates/runner/tests/support/e2e_integrity.rs` proves the smell concretely.
  - `VerifyJobResponse` contains only `job_id` and `status`
  - `VerifyCorrectnessAudit::new(...)` merges that response with `VerifyLogAudit::from_logs(...)`
- The current raw-table feature does not exist.
  - config has no gate
  - service exposes no inspection route
  - there is no narrow service-owned reader for source/destination table JSON

## Improve-Code-Boundaries Focus

- Primary smell: wrong-placeism.
  - correctness facts are born in the Go service but interpreted in Rust from container logs
  - the runtime has become a courier for internals instead of owning one result boundary
- Secondary smell: mixed responsibilities.
  - the current `job` type is lifecycle state only
  - the harness owns semantic result reconstruction it should not own
- Tertiary smell: one shared shape, one render.
  - the service needs one canonical typed verify-result aggregate
  - JSON rendering for `GET /jobs/{job_id}` and Rust result decoding should both reuse that one shape instead of independent ad hoc interpretations
- Desired end state:
  - one canonical per-job verify result aggregate inside `verifyservice`
  - one renderer for `GET /jobs/{job_id}`
  - one narrow raw-table reader owned by the service config/connection boundary
  - zero correctness assertions derived from container logs

## Public Contract Decisions

### Job Result JSON

- Keep `GET /jobs/{job_id}` as the result endpoint.
- Extend the JSON payload from:
  - `{ "job_id": "...", "status": "..." }`
- To a structured shape along these lines:
  - `job_id`
  - `status`
  - `result`
- `result` should be omitted while a job is still `running` unless there is already meaningful partial data worth exposing.
  - If partial data is exposed during `running`, it must use the same final shape and remain explicitly partial
- The result payload should be explicit and typed, for example:
  - `matched_tables`
  - `mismatched_tables`
  - `table_definitions`
  - `summary`
  - `failure`
- Preferred flattened JSON shape:
  - `result.table_summaries[]`
    - `schema`
    - `table`
    - `num_verified`
    - `num_success`
    - `num_missing`
    - `num_mismatch`
    - `num_column_mismatch`
    - `num_extraneous`
    - `num_live_retry`
  - `result.table_definition_mismatches[]`
    - `schema`
    - `table`
    - `message`
  - `result.mismatch_tables[]`
    - one entry per table known to have any mismatch condition
  - `result.completed`
    - boolean derived from lifecycle state, only if useful to keep Rust-side assertions simple
- Do not expose free-form status log lines as the correctness contract.
  - `StatusReport.Info` remains useful for logs, not for correctness assertions

### Raw Table JSON

- Add one narrow read endpoint rather than two duplicated surfaces.
  - preferred route: `POST /tables/raw`
- Request shape:
  - `database`
    - enum: `source` or `destination`
  - `schema`
  - `table`
- No other request fields are allowed.
  - no SQL
  - no filters
  - no ordering
  - no limit unless execution proves a hard cap is required and the task allows it
- Response shape:
  - `database`
  - `schema`
  - `table`
  - `columns`
  - `rows`
- Preferred row shape:
  - `rows` is an array of JSON objects keyed by returned column name
  - `columns` preserves the output order explicitly so consumers do not depend on map ordering
- Failure contract:
  - feature disabled in config:
    - fail closed with explicit HTTP error
  - unknown `database`, invalid schema/table name, unreadable table, unsupported value conversion:
    - fail loudly with explicit HTTP error
  - never drop fields silently to force JSON encoding to succeed

## Config Decisions

- Extend verify-service config with an explicit operator-only gate, for example:
  - `verify.raw_table_output.enabled: false`
- Keep it under `verify` rather than adding a second top-level concern.
  - the feature depends on the existing config-owned source/destination connections
- Default must remain `false`.
- Validation should accept absent config as disabled and reject malformed explicit values.
- The HTTP layer must check the gate before attempting any table read.

## Proposed Code Shape

- `cockroachdb_molt/molt/verifyservice/service.go`
  - keep route wiring and job lifecycle transitions
  - add `POST /tables/raw`
  - stop treating `job` as lifecycle state only
- `cockroachdb_molt/molt/verifyservice/job.go`
  - move job lifecycle state plus response rendering here
- `cockroachdb_molt/molt/verifyservice/result.go`
  - own the canonical per-job verify-result aggregate
  - own aggregation from `inconsistency.ReportableObject`
- `cockroachdb_molt/molt/verifyservice/raw_table.go`
  - own request validation, identifier-safe query construction, and JSON row materialization
- `cockroachdb_molt/molt/verifyservice/config.go`
  - add raw-table gate config plus validation/default behavior
- `cockroachdb_molt/molt/verifyservice/config_test.go`
  - add gate default/enabled validation coverage
- `cockroachdb_molt/molt/verifyservice/http_test.go`
  - drive both the job-result JSON contract and raw-table HTTP contract through TDD
- `crates/runner/tests/support/e2e_integrity.rs`
  - replace log-derived correctness with typed response-derived correctness
- `crates/runner/tests/support/verify_image_harness.rs`
  - stop reading container logs for correctness
  - decode the richer job JSON instead

## Internal Type Decisions

- Introduce one canonical aggregate, for example:
  - `jobResult`
- It should store the semantic result, not HTTP transport details.
- Suggested internal fields:
  - `tableSummaries map[tableKey]tableSummary`
  - `tableDefinitionMismatches map[tableKey][]string`
  - `mismatchTables map[tableKey]struct{}`
- `tableKey` should be one internal owner for `schema` plus `table`.
  - this avoids duplicated stringly concatenation between service and Rust layers
- `recordReport` should update that aggregate directly from typed `inconsistency` values.
  - `SummaryReport` updates the latest per-table summary
  - `MismatchingTableDefinition` records a table-level mismatch
  - row-level mismatch objects mark the table as mismatched
- HTTP DTOs should be rendered from the aggregate on read.
  - do not let the mutable aggregate double as the JSON DTO

## Raw Table Reader Decisions

- Add a service-owned reader abstraction, for example:
  - `type TableReader interface { ReadRawTable(ctx context.Context, side rawTableSide, schema, table string) (RawTableResult, error) }`
- Default implementation should:
  - derive source/destination connection strings from config only
  - connect through the existing `dbconn.Connect` boundary
  - clone or create a dedicated connection for the one read
- Query construction must be identifier-safe.
  - schema and table names must be validated as identifiers, not interpolated blindly
  - if proper identifier escaping inside the retained Go parser utilities is not cleanly available, switch back to `TO BE VERIFIED` rather than shipping stringly SQL
- JSON materialization must be explicit.
  - preserve column names
  - preserve nulls
  - support JSON-representable scalar/array/object values
  - fail loudly on values that cannot be represented honestly

## Rust Harness Boundary Changes

- `VerifyJobResponse` in `e2e_integrity.rs` should grow to match the new HTTP payload.
- `VerifyCorrectnessAudit::new(...)` should take only the typed HTTP response.
  - remove the `logs: String` input entirely
- Delete `VerifyLogAudit` and the log-parsing helpers once the HTTP contract covers the needed assertions.
- `verify_image_harness.rs` should keep docker logs only for failure diagnostics.
  - logs remain in panic messages when the container fails
  - logs stop being the asserted correctness input

## TDD Slices

### Slice 1: Tracer Bullet For Structured Job Results

- [x] RED: add one failing Go HTTP test proving `GET /jobs/{job_id}` returns structured table summary data after a successful verify run
- [x] GREEN: add the smallest canonical result aggregate plus JSON rendering needed to make that test pass
- [x] REFACTOR: keep lifecycle state and semantic verify results separate so the mutable store is not the HTTP DTO

### Slice 2: Mismatch Result Coverage

- [x] RED: add one failing Go HTTP test proving mismatch cases appear through the job JSON contract without reading logs
- [x] GREEN: update result aggregation so row-level and table-definition mismatches are retained in the canonical aggregate
- [x] REFACTOR: collapse all mismatch-table marking into one internal owner instead of scattering ad hoc flags

### Slice 3: Rust Correctness Boundary Migration

- [x] RED: change one Rust correctness-support test so it expects the richer job JSON and no longer accepts log-derived correctness
- [x] GREEN: update `VerifyJobResponse`, `VerifyCorrectnessAudit`, and `VerifyImageHarness` to consume typed HTTP results only
- [x] REFACTOR: delete `VerifyLogAudit` and keep docker logs for diagnostics only

### Slice 4: Raw-Table Config Gate

- [x] RED: add failing Go config and HTTP tests proving raw-table output is disabled by default and fails closed when the gate is off
- [x] GREEN: add the config gate plus HTTP gate check
- [x] REFACTOR: keep the gate owned by config validation, not sprinkled as magic booleans through handlers

### Slice 5: Source-Side Raw Table Read

- [x] RED: add one failing Go HTTP test proving `POST /tables/raw` can return full JSON rows for a selected source table when enabled
- [x] GREEN: add the narrow raw-table reader path for `source`
- [x] REFACTOR: keep identifier validation and DB selection in one service-owned module

### Slice 6: Destination-Side Raw Table Read

- [x] RED: add one failing Go HTTP test proving the same endpoint works for `destination`
- [x] GREEN: extend the reader cleanly to the destination side without widening the caller contract
- [x] REFACTOR: keep one request/response schema and one reader boundary instead of side-specific handler duplication

### Slice 7: Loud Failure On Unsupported Values Or Requests

- [x] RED: add failing Go tests for invalid request shapes, unsupported identifiers, and non-JSON-safe value handling
- [x] GREEN: return explicit HTTP errors without silent field drops or error swallowing
- [x] REFACTOR: keep conversion and validation errors typed and local to the raw-table module

### Slice 8: Full Validation

- [x] RED: run `make check`, `make lint`, and `make test`, fixing the first failing lane at a time
- [x] GREEN: continue until all required default lanes pass cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to ensure the result aggregate and raw-table reader actually flattened the split boundary rather than layering more DTO glue on top

## TDD Guardrails For Execution

- One failing test at a time.
- Do not write all job-result tests first and all implementation second.
- Prefer public-interface tests:
  - verify-service HTTP tests
  - Rust typed response contract tests
- Do not preserve the log-scraping correctness path as a fallback.
- Do not add a generic SQL endpoint.
- Do not allow HTTP callers to pass connection strings, TLS paths, verify modes, or ad hoc SQL.
- Do not swallow conversion or query errors.
  - if execution uncovers an unrelated pre-existing error-swallowing path that cannot be fixed within this task, record it as an `add-bug` task instead of ignoring it

## Boundary Review Checklist

- [x] `GET /jobs/{job_id}` is the single correctness result contract for verify-image-backed tests
- [x] `verifyservice` retains typed verify findings instead of dropping them in `recordReport`
- [x] Rust correctness support no longer parses container logs to determine match or mismatch
- [x] raw-table reads are config-gated and disabled by default
- [x] the raw-table HTTP contract is narrow and does not permit caller-controlled connection or SQL details
- [x] raw-table JSON never drops unsupported fields silently
- [x] one canonical result aggregate feeds JSON rendering instead of duplicate semantic models in Go and Rust

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [ ] `make test-long` only if execution proves this task changed the long-lane selection or the task explicitly requires it
- [x] One final `improve-code-boundaries` pass after the required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after the required lanes pass

Plan path: `.ralph/tasks/story-18-verify-http-image/11-task-add-config-gated-raw-source-and-destination-table-json-output-to-verify-http_plans/2026-04-20-verify-http-json-results-and-raw-table-output-plan.md`

NOW EXECUTE
