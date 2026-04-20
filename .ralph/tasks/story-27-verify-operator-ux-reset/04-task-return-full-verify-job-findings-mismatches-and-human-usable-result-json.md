## Task: Return full verify job findings, mismatches, and human-usable result JSON <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Make the verify job result surface return the actual verification findings instead of a coarse “error?” style summary. The higher order goal is to let an operator use `GET /jobs/{job_id}` as the real truth source for correctness, mismatches, and failure analysis without falling back to container logs or ad hoc database inspection.

Current product gap from the 2026-04-20 user review:
- the job does not give the actual results
- the response surface currently does not make mismatches obvious or useful
- operators need real mismatch details and not just a binary error field

In scope:
- extend the verify job result model so final job JSON includes structured verification findings, mismatch summaries, and detailed failure context
- ensure the returned result distinguishes true mismatches from infrastructure or execution failures
- include enough source-versus-destination detail to explain what was wrong, such as per-table findings, counts, mismatch summaries, or other concrete compare output available from the underlying verify run
- preserve machine-readable structure so callers do not need to parse free-form logs
- update harnesses and contracts so tests assert the HTTP result payload directly
- update docs and curl examples to show what a useful success result and a useful mismatch result look like

Out of scope:
- exposing arbitrary SQL query endpoints
- inventing data that the underlying verify run does not produce
- weakening request validation or security boundaries to fetch more detail

Decisions already made:
- the verify HTTP job result must be the supported operator contract, not an afterthought behind logs
- a result payload that only says “error” or equivalent is insufficient
- mismatch information should be returned in structured JSON and not hidden in container logs
- this task should build on and, where needed, deepen the earlier structured-result work from `story-18-verify-http-image/11-task-add-config-gated-raw-source-and-destination-table-json-output-to-verify-http.md` so the operator result surface becomes genuinely usable

Relevant files and boundaries:
- `cockroachdb_molt/molt/verifyservice/result.go`
- `cockroachdb_molt/molt/verifyservice/result_test.go`
- `cockroachdb_molt/molt/verifyservice/service.go`
- `cockroachdb_molt/molt/verifyservice/http_test.go`
- `cockroachdb_molt/molt/verifyservice/verify_runner.go`
- `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
- `crates/runner/tests/verify_job_result_contract.rs`
- `crates/runner/tests/support/verify_image_harness.rs`
- `crates/runner/tests/support/e2e_integrity.rs`
- `README.md`

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers success results, mismatch results, and infrastructure-failure results through the HTTP job JSON contract
- [ ] Final job JSON includes structured findings that explain what was wrong rather than only a coarse `error` field
- [ ] Mismatch-driven failures are clearly separated from transport/auth/config/process failures
- [ ] The Rust verify-image harness and result-contract tests assert the richer HTTP result payload directly rather than depending on log scraping for core correctness details
- [ ] Docs and curl examples show at least one useful mismatch result payload and one useful success result payload
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
