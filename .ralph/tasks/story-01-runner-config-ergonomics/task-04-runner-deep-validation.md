## Task: Add deep validation mode to runner validate-config <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete

**Goal:** Extend `runner validate-config` with an optional deep validation mode that connects to configured PostgreSQL destinations, verifies credentials work, and confirms that mapped tables exist. Currently `validate-config` only checks syntax and file paths. Users discover connectivity issues only after starting the long-lived runtime.

**In scope:**
- Add a `--deep` flag (or similar) to the `validate-config` subcommand in `crates/runner/src/lib.rs`.
- When `--deep` is passed, attempt to connect to each mapping's destination using `PgConnection::connect_with`.
- Verify that each mapped table exists in the destination catalog.
- Report failures with mapping ID and endpoint context.
- Keep the default `validate-config` behavior fast and offline (no network calls without `--deep`).
- Update README to document `--deep` and when to use it.
- Add tests in `crates/runner/tests/config_contract.rs`.

**Out of scope:**
- Actually starting the runtime.
- Writing to destination databases.
- Checking CockroachDB source connectivity (runner does not connect to CockroachDB).
- Deep validation as the default behavior.
- Changing the `setup-sql` command behavior.

**End result:**
A user can run:
```bash
runner validate-config --config /config/runner.yml --deep
```
and get immediate feedback like:
```
config valid: config=/config/runner.yml mappings=2 webhook=0.0.0.0:8443 tls=enabled deep=ok
```
or:
```
config: failed to connect mapping `app-a` to `pg-a.example.internal:5432/app_a`: connection refused
```
</description>

<acceptance_criteria>
- [x] `validate-config --deep` connects to each destination and verifies connectivity
- [x] `validate-config --deep` verifies that each mapped table exists in the destination
- [x] `validate-config` without `--deep` remains fast and does not make network calls
- [x] Deep validation errors include mapping ID and endpoint label
- [x] Deep validation works with both decomposed and URL-based destination configs
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite)
- [x] `make lint` — passes cleanly
</acceptance_criteria>

<plan>.ralph/tasks/story-01-runner-config-ergonomics/task-04-runner-deep-validation_plans/2026-04-25-runner-deep-validation-plan.md</plan>
