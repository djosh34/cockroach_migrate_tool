## Bug: Verify HTTP allows warning-only insecure listener modes <status>done</status> <passes>true</passes> <priority>ultra high</priority>

<description>
The verify HTTP audit found that the listener accepts insecure remote-service modes such as plain HTTP and no client authentication. The CLI only prints `warning: no extra built-in protection is being provided by the verify service` and still treats those configurations as valid.

This was detected during audit pass 5 while reviewing `cockroachdb_molt/molt/verifyservice/config.go`, `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`, and `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`.

This is security-sensitive because the service is remotely triggered and exposes verification control plus operational data. Leaving transport protection and caller authentication as warning-only behavior turns deployment discipline into the sole enforcement mechanism.

Audit pass: 5

Affected files or boundaries:
- `cockroachdb_molt/molt/verifyservice/config.go`
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
- listener transport and client-auth policy boundary

First Red test to add:
- add a config-validation or CLI integration test proving remote-service startup fails when the listener is configured for plain HTTP or HTTPS without the required caller-auth policy.
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
- [x] `make test-long` was not required because the impacted verify-image coverage lives in the default `make test` lane, and that lane now passes cleanly
</acceptance_criteria>

<plan>.ralph/tasks/bugs/bug-verify-http-allows-warning-only-insecure-listener-modes_plans/2026-04-19-verify-http-insecure-listener-modes-plan.md</plan>
