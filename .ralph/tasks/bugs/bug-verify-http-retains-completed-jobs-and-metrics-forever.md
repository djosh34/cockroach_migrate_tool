## Bug: Verify HTTP retains completed jobs and metrics forever <status>not_started</status> <passes>false</passes> <priority>high</priority>

<description>
The verify HTTP audit found that completed jobs are never pruned from `Service.jobs`. Each finished job keeps its full in-memory progress snapshot, including status messages, summary events, mismatch records, and error strings. `/metrics` then iterates every remembered job on every scrape.

This was detected during audit pass 3 while reviewing `cockroachdb_molt/molt/verifyservice/service.go`, `cockroachdb_molt/molt/verifyservice/progress.go`, and `cockroachdb_molt/molt/verifyservice/metrics.go`.

This is security-sensitive because a remotely reachable caller can create a long-lived memory and scrape-cost amplification path by submitting many jobs over time. The issue is worse than simple bookkeeping growth because the retained state is also exposed through the metrics surface.

Audit pass: 3

Affected files or boundaries:
- `cockroachdb_molt/molt/verifyservice/service.go`
- `cockroachdb_molt/molt/verifyservice/progress.go`
- `cockroachdb_molt/molt/verifyservice/metrics.go`
- job retention and metrics snapshot boundaries

First Red test to add:
- add an integration-style test proving the service prunes or caps completed jobs after a defined retention policy and that `/metrics` no longer emits historical series beyond that boundary.
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
