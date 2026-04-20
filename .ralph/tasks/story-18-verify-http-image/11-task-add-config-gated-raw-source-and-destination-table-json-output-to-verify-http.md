## Task: Complete the verify HTTP JSON read surface with structured job results and config-gated raw table output <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Make the Go verify-service a coherent JSON read surface for both machine-readable verification results and operator debugging. This task now owns the full verify HTTP result boundary: `GET /jobs/{job_id}` must return structured match/mismatch findings instead of forcing clients to scrape logs, and the service must also support config-gated raw source/destination table JSON output for operator inspection when explicitly enabled.

Historical split being removed:
- this task absorbs `.ralph/tasks/story-09-verification-cutover/04-task-return-structured-verify-results-via-http-json.md`
- that older task and this task were both changing the verify-service JSON read surface, but from two angles: structured job results and raw table inspection
- keeping them separate left one product boundary with two backlog owners, which is unnecessary churn for the same HTTP surface

Current product gap:
- the verify-service currently exposes only a narrow job-control HTTP surface
- `GET /jobs/{job_id}` currently returns coarse lifecycle state and does not expose the structured verification findings clients actually need
- `verifyservice/service.go` drops reported verify findings on the floor via `recordReport`
- the Rust E2E harness currently works around that gap by parsing JSON container logs in `crates/runner/tests/support/e2e_integrity.rs` after polling HTTP job status
- there is no supported operator path to ask the verify image for full raw source and destination table outputs in JSON when debugging a mismatch or validating completeness
- operators who need to inspect actual source-versus-destination data currently have to rely on database access or ad hoc test/log paths instead of the verify HTTP surface

In scope:
- extend the Go verify-service job result model to retain structured verification findings needed by clients
- expose machine-readable match/mismatch output in the HTTP JSON returned by `GET /jobs/{job_id}`
- cover both matched-table summaries and mismatch cases in the returned JSON
- keep the current lifecycle status contract (`running`, `succeeded`, `failed`, `stopped`) while adding structured result data
- explicitly remove the current log-backed correctness assertion path for selected-table completeness and switch it back to the verify HTTP JSON contract
- update the Rust verify-image harness and E2E integrity helpers to consume the HTTP JSON result instead of parsing container logs for correctness
- add a verify-service config switch that explicitly enables or disables raw table-output querying
- keep the raw-table feature disabled by default unless the config enables it explicitly
- when enabled, expose an HTTP path or paths that let an operator request full raw JSON output for a selected table from the CockroachDB source and the PostgreSQL destination
- support returning raw rows in JSON regardless of the table’s column shape, as long as the values can be represented in JSON
- define clear request and response schemas for the raw table-output feature
- cover both source-side and destination-side output retrieval
- define loud failure behavior for unsupported, unreadable, or non-JSON-representable values instead of silently dropping fields
- preserve strict request validation and the existing config-owned connection boundary; HTTP callers must not be able to override DB URLs, TLS material, or verify modes

Out of scope:
- redesigning the entire verify algorithm
- streaming progress over a new protocol
- building a generic SQL query endpoint
- allowing arbitrary WHERE clauses, ORDER BY fragments, or free-form SQL from HTTP callers
- adding a separate persistence layer for historical job storage beyond the current in-memory service model unless strictly required by the new result payload

Decisions already made:
- this task is about the Go verify-service HTTP API under `cockroachdb_molt/molt`
- it should make the HTTP result surface the real contract again instead of process logs
- the resulting job JSON must be structured and machine-readable; clients should not have to parse log lines to determine which tables matched or mismatched
- raw source and destination table outputs must be JSON and operator-queryable through the Go verify-service when explicitly enabled
- the raw-table feature must be guarded by explicit config so deployments can enable or disable it intentionally
- if the feature is disabled in config, the HTTP surface must fail closed and not leak table contents
- tests should assert the HTTP result contract directly, and the Rust harness should stop depending on container log parsing for selected-table correctness
- request validation, HTTPS, mTLS, and config ownership remain part of the supported security posture

Relevant files:
- `cockroachdb_molt/molt/verifyservice/config.go`
- `cockroachdb_molt/molt/verifyservice/config_test.go`
- `cockroachdb_molt/molt/verifyservice/service.go`
- `cockroachdb_molt/molt/verifyservice/http_test.go`
- `cockroachdb_molt/molt/verifyservice/metrics.go`
- `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- `cockroachdb_molt/molt/dbconn/*.go`
- `crates/runner/tests/support/verify_image_harness.rs`
- `crates/runner/tests/support/e2e_integrity.rs`

</description>


<acceptance_criteria>
- [x] Red/green TDD covers `GET /jobs/{job_id}` returning structured verification findings for successful runs, including matched-table summaries and mismatch counts
- [x] Red/green TDD covers mismatch cases through the HTTP JSON contract without relying on container log parsing
- [x] The Go verify-service keeps its current lifecycle states while augmenting job JSON with structured result data
- [x] The Rust verify-image harness and E2E integrity helpers use HTTP JSON results rather than parsing verify container logs for correctness
- [x] No selected-table completeness or mismatch assertion in the verify-image-backed test harness depends on container log scraping; those assertions are driven by the verify HTTP result payload
- [x] Red/green TDD covers the raw-table config gate being disabled by default and explicitly enabled through verify-service config
- [x] Red/green TDD covers HTTP retrieval of full raw JSON output for a selected CockroachDB source table and a selected PostgreSQL destination table when the feature is enabled
- [x] The raw-table feature fails closed when disabled in config and does not expose table contents accidentally
- [x] The raw-table feature preserves the config-owned connection boundary and does not allow HTTP callers to inject arbitrary connection or SQL details
- [x] Raw table outputs are returned as JSON for arbitrary table shapes without silently dropping unsupported fields or errors
- [x] The request and response schema for both job-result JSON and raw-table-output querying is explicit and documented through tests
- [x] The HTTP response contract remains explicit and safe; request validation, error handling, and metrics contracts are not silently weakened
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-18-verify-http-image/11-task-add-config-gated-raw-source-and-destination-table-json-output-to-verify-http_plans/2026-04-20-verify-http-json-results-and-raw-table-output-plan.md</plan>
