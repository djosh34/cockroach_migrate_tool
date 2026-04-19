## Bug: Verify HTTP exposes job results and metrics without auth <status>not_started</status> <passes>false</passes> <priority>ultra high</priority>

<description>
The verify HTTP audit found that `GET /jobs/{job_id}` and `GET /metrics` expose operational details to any caller on the listener. The current behavior includes job IDs, timestamps, failure reasons, mismatch details, source and destination database names, schema names, table names, and mismatch counts, with no authentication or authorization layer in the service itself.

This was detected during audit pass 4 while reviewing `cockroachdb_molt/molt/verifyservice/service.go`, `cockroachdb_molt/molt/verifyservice/progress.go`, `cockroachdb_molt/molt/verifyservice/metrics.go`, and the existing integration tests in `cockroachdb_molt/molt/verifyservice/http_test.go`.

This is security-sensitive because the service is a remote control plane for database verification. Even without command execution, unauthenticated callers can learn database naming, table layout, mismatch activity, and job history from the read endpoints.

Audit pass: 4

Affected files or boundaries:
- `cockroachdb_molt/molt/verifyservice/service.go`
- `cockroachdb_molt/molt/verifyservice/progress.go`
- `cockroachdb_molt/molt/verifyservice/metrics.go`
- public read surfaces for `/jobs/{job_id}` and `/metrics`

First Red test to add:
- add an integration test proving unauthorized requests to `/jobs/{job_id}` and `/metrics` are rejected, or that the public surface is reduced to an explicitly safe subset with no sensitive labels or mismatch details.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [ ] I created a Red unit and/or integration test that captures the bug
- [ ] I made the test green by fixing
- [ ] I manually verified the bug, and created a new Red test if not working still
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
