## Task: Add deep validation mode to runner validate-config <status>not_started</status> <passes>false</passes>

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
- [ ] `validate-config --deep` connects to each destination and verifies connectivity
- [ ] `validate-config --deep` verifies that each mapped table exists in the destination
- [ ] `validate-config` without `--deep` remains fast and does not make network calls
- [ ] Deep validation errors include mapping ID and endpoint label
- [ ] Deep validation works with both decomposed and URL-based destination configs
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
