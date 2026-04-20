## Bug: Runner test port selection flakes and can fail webhook bind during parallel test runs <status>not_started</status> <passes>false</passes> <priority>medium</priority>

<description>
The runner test suite still has a time-of-check/time-of-use port-allocation race.

During `make test` on 2026-04-20, `crates/runner/tests/reconcile_contract.rs` failed in
`run_reconciles_each_mapping_into_only_its_own_tables_inside_a_shared_destination_database`
because the child runner process exited before healthz with:

- `runner runtime starting`
- `webhook runtime: failed to bind webhook listener on \`127.0.0.1:44321\``

The same test passed immediately in isolated rerun, and the full suite passed on rerun, which
points to flaky port reservation rather than a deterministic runtime regression. The test harnesses
currently pick a free port by binding `127.0.0.1:0`, reading the chosen port, closing the listener,
and later asking the real runner process to bind that port. Parallel tests can steal that port in
between.

In scope:
- add a Red test or harness-level reproduction that captures the bind race honestly
- replace the TOCTOU `pick_unused_port()` pattern in runner test support with an ownership-safe port
  reservation strategy
- prove the reconcile/webhook runner contract tests stop failing from transient webhook bind races

Out of scope:
- changing production webhook bind behavior
- weakening healthz assertions or retrying away the race in tests
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
