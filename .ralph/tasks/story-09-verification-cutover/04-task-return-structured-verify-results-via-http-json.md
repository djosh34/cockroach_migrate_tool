## Task: Return structured verify matches and mismatches via HTTP JSON <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Change the Go verify-service HTTP boundary in `cockroachdb_molt/molt/verifyservice` so clients can read structured verification findings directly from JSON responses instead of scraping process logs. The higher order goal is to make the dedicated verify image a coherent machine-readable API boundary for repeated parity checks and final cutover verification.

Current problem discovered from code and tests:
- the dedicated verify image is the Go `verify-service` image, not a Rust runtime surface
- `GET /jobs/{job_id}` currently returns only coarse lifecycle state (`job_id`, `status`)
- `verifyservice/service.go` drops reported verify findings on the floor via `recordReport`
- the Rust E2E harness currently works around this by parsing JSON container logs in `crates/runner/tests/support/e2e_integrity.rs` after polling the HTTP job status
- the current log-backed correctness path was introduced during verify HTTP read-surface hardening, which moved test completeness assertions away from the HTTP result payload and back onto verify container logs
- this means the HTTP API does not expose the actual matches/mismatches it exists to report, which makes the boundary incomplete and forces log-coupled consumers

In scope:
- extend the Go verify-service job result model to retain structured verification findings needed by clients
- expose machine-readable match/mismatch output in the HTTP JSON returned by `GET /jobs/{job_id}`
- cover both matched-table summaries and mismatch cases in the returned JSON
- keep the current lifecycle status contract (`running`, `succeeded`, `failed`, `stopped`) while adding structured result data
- explicitly remove the current log-backed correctness assertion path for selected-table completeness and switch it back to the verify HTTP JSON contract
- update the Rust verify-image harness and E2E integrity helpers to consume the HTTP JSON result instead of parsing container logs for correctness
- preserve strict request validation and the existing security posture around config ownership, HTTPS, and mTLS

Out of scope:
- redesigning the entire verify algorithm
- streaming progress over a new protocol
- adding a separate persistence layer for historical job storage beyond the current in-memory service model unless strictly required by the new result payload

Decisions already made:
- this task is about the Go verify-service HTTP API under `cockroachdb_molt/molt`
- it should mitigate the current bad contract where HTTP omits the verification findings and logs become the real API
- the resulting JSON must be structured and machine-readable; clients should not have to parse log lines to determine which tables matched or mismatched
- tests should assert the new HTTP result contract directly, and the Rust harness should stop depending on container log parsing for selected-table correctness
- this task must explicitly reverse the current testing boundary where completeness is inferred from verify-image logs rather than from HTTP job output

Relevant files:
- `cockroachdb_molt/molt/verifyservice/service.go`
- `cockroachdb_molt/molt/verifyservice/http_test.go`
- `cockroachdb_molt/molt/verifyservice/metrics.go`
- `crates/runner/tests/support/verify_image_harness.rs`
- `crates/runner/tests/support/e2e_integrity.rs`

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers `GET /jobs/{job_id}` returning structured verification findings for successful runs, including matched-table summaries and mismatch counts
- [ ] Red/green TDD covers mismatch cases through the HTTP JSON contract without relying on container log parsing
- [ ] The Go verify-service keeps its current lifecycle states while augmenting job JSON with structured result data
- [ ] The Rust verify-image harness and E2E integrity helpers use HTTP JSON results rather than parsing verify container logs for correctness
- [ ] No selected-table completeness or mismatch assertion in the verify-image-backed test harness depends on container log scraping; those assertions are again driven by the verify HTTP result payload
- [ ] The HTTP response contract remains explicit and safe; request validation, error handling, and metrics contracts are not silently weakened
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
