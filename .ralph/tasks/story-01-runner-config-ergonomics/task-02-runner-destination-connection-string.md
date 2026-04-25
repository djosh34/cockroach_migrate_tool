## Task: Allow runner destination to accept PostgreSQL connection strings <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete

**Goal:** Add an optional `url` field to `mappings[].destination` in the runner config as an alternative to the decomposed `host`/`port`/`database`/`user`/`password`/`tls` fields. Most PostgreSQL users already have a working connection string from existing tooling, and the current decomposed format is verbose and unfamiliar.

**In scope:**
- Add optional `url` string field to `RawPostgresTargetConfig` in `crates/runner/src/config/parser.rs`.
- When `url` is provided, parse it into `PostgresTargetConfig` fields internally using `sqlx::postgres::PgConnectOptions::parse` or similar.
- When `url` is not provided, keep existing decomposed field behavior exactly.
- Reject configs that provide both `url` and decomposed fields (or at least define clear precedence and document it).
- Support TLS parameters inside the connection string (`sslmode`, `sslrootcert`, `sslcert`, `sslkey`).
- Update README to show both styles with the connection string as the recommended concise option.
- Update tests in `crates/runner/tests/config_contract.rs`.

**Out of scope:**
- Removing decomposed fields.
- Changing existing config validation logic for decomposed fields.
- Changing the verify service URL-based config (it already uses URLs).
- Adding connection string support to `setup-sql` configs.

**End result:**
A user can write either:
```yaml
# concise style (new)
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=/config/certs/ca.crt
```
or keep the existing decomposed style.
</description>

<acceptance_criteria>
- [x] Config parser accepts `mappings[].destination.url` as a valid PostgreSQL connection string
- [x] Config parser rejects invalid or malformed connection strings with a clear error
- [x] Config parser rejects configs that specify both `url` and decomposed fields simultaneously
- [x] `validate-config` works with `url`-based destinations
- [x] Runtime connects successfully using `url`-based destinations
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite)
- [x] `make lint` — passes cleanly
</acceptance_criteria>

<plan>.ralph/tasks/story-01-runner-config-ergonomics/task-02-runner-destination-connection-string_plans/2026-04-25-runner-destination-connection-string-plan.md</plan>
