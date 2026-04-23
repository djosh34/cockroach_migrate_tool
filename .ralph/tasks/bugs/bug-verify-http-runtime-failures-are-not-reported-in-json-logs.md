## Bug: Verify HTTP runtime failures are not reported in JSON logs <status>not_started</status> <passes>false</passes> <priority>high</priority>

<description>
During the receive-mail investigation for "How to use it?", the real verify HTTP service was run locally with `--log-format json` and exercised through curl against real PostgreSQL databases.

The HTTP job result correctly returned structured failures for:

- missing source database: `category=source_access`, `code=connection_failed`
- missing source table permission: `category=verify_execution`, `code=verify_failed`
- passwordless URL rejected by the database: `category=source_access`, `code=connection_failed`

However, the verify-service JSON log files for those runs only contained the `runtime.starting` line and did not record the job failure details. This means operators can get the error back through `GET /jobs/{job_id}`, but the same actionable error is not reported in the service logs.

Reproduction evidence from the investigation:

- bad database response included `FATAL: database "verify_http_missing" does not exist (SQLSTATE 3D000)`, while `bad-database.log` only had `runtime.starting`
- bad permission response included `permission denied for table accounts (SQLSTATE 42501)`, while `no-permission.log` only had `runtime.starting`
- passwordless rejected response included `password authentication failed for user "postgres" (SQLSTATE 28P01)`, while `passwordless.log` only had `runtime.starting`

The expected behavior is that every failed verify job logs the structured failure category, code, message, and details at error level, without leaking secrets.
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
- [ ] Failed source connection jobs are emitted to JSON logs with category, code, message, and details
- [ ] Failed verify execution jobs are emitted to JSON logs with category, code, message, and details
- [ ] Logged failures do not expose database passwords or other secret material
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
