## Bug: Verify HTTP request body size is unbounded <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
The verify HTTP audit found that `POST /jobs` and `POST /stop` decode directly from the full request body without a size cap. The new strict decoder rejects unknown fields and trailing documents, but it still allows arbitrarily large request bodies to be read into memory before validation completes.

This was detected during audit pass 1 while reviewing `cockroachdb_molt/molt/verifyservice/service.go` and the request-boundary tests in `cockroachdb_molt/molt/verifyservice/http_test.go`.

This is security-sensitive because the service is remotely reachable and the request shape is intentionally small. A hostile client can force avoidable parser and allocation work with an oversized body even though the handler only needs a tiny JSON object.

Audit pass: 1

Affected files or boundaries:
- `cockroachdb_molt/molt/verifyservice/service.go`
- request decode boundary for `POST /jobs`
- request decode boundary for `POST /stop`

First Red test to add:
- add an HTTP integration test proving `POST /jobs` returns `413` or another explicit rejection when the body exceeds the configured maximum size, and that the runner never starts.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
- [x] I made the test green by fixing
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only) — not required because this task did not change the long-lane selection or e2e boundary
</acceptance_criteria>

<plan>.ralph/tasks/bugs/bug-verify-http-request-body-size-is-unbounded_plans/2026-04-19-verify-http-request-body-limit-plan.md</plan>
