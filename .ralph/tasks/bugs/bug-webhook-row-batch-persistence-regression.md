## Bug: Webhook row-batch persistence contract fails with HTTP 501 instead of 200 <status>done</status> <passes>true</passes> <priority>high</priority>

<description>
Detected on 2026-04-18 during a reporting audit by running `make test`.

The default workspace test suite currently fails in `crates/runner/tests/webhook_contract.rs` on `run_persists_insert_row_batches_before_returning_200`.
Observed failure:

- expected HTTP status: `200`
- actual HTTP status: `501`

This means the repository currently does not satisfy the claimed destination-ingest behavior for persisting row batches before acknowledging the webhook request.
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
- [x] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
