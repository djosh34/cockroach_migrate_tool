# Plan: Allow Runner Destination To Accept PostgreSQL Connection Strings

## References

- Task:
  - `.ralph/tasks/story-01-runner-config-ergonomics/task-02-runner-destination-connection-string.md`
- Current runner config boundary:
  - `crates/runner/src/config/parser.rs`
  - `crates/runner/src/config/mod.rs`
  - `crates/runner/src/error.rs`
- Current runner config contract coverage:
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/support/runner_public_contract.rs`
- Current operator docs and fixtures:
  - `README.md`
  - `crates/runner/tests/fixtures/readme-runner-config.yml`
  - `crates/runner/tests/fixtures/valid-runner-config.yml`
  - `crates/runner/tests/fixtures/container-runner-config.yml`
- Supporting upstream library seam:
  - local `sqlx-postgres` 0.8.6 source under `~/.cargo/registry/.../sqlx-postgres-0.8.6/src/options`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient product approval for this planning turn.
- This turn is planning-only because the task had no linked plan artifact yet.
- This is greenfield work.
  - keep the existing decomposed YAML path working
  - do not add compatibility shims beyond the explicit dual-shape config contract
  - delete any awkward intermediate conversion code rather than preserving it
- `sqlx` 0.8.6 already parses PostgreSQL URLs with:
  - `sslmode`
  - `sslrootcert`
  - `sslcert`
  - `sslkey`
- `sqlx::ConnectOptions::to_url_lossy()` is available, which gives a canonical contract surface for comparing two destination targets after normalization.
- If the first RED slice proves that URL parsing cannot be normalized into one honest `PostgresTargetConfig` boundary without keeping duplicated source-of-truth state, switch this plan back to `TO BE VERIFIED` and stop immediately.
- If the first RED slice proves that URL parsing would silently inherit environment-only connection material in a way that muddies the runner config contract, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - `validate-config` accepts a URL-only destination config
  - malformed URLs fail with a clear config error
  - mixed URL plus decomposed destination fields fail loudly
  - runtime can really connect with a URL-based destination, not just parse it
  - README presents the URL form as the concise recommended operator surface
- Lower-priority concerns:
  - preserving every decomposed example as the primary documented style
  - adding connection-string support to unrelated config files such as `setup-sql`

## Current State Summary

- `RawPostgresTargetConfig` is decomposed-only today:
  - `host`
  - `port`
  - `database`
  - `user`
  - `password`
  - optional `tls`
- `PostgresTargetConfig` also stores the destination as decomposed fields and rebuilds `PgConnectOptions` on every `connect_options()` call.
- That means the parser boundary is doing too much shape ownership:
  - YAML shape and runtime connection contract are fused together
  - adding URL input would otherwise force more branching into `parser.rs`
  - `same_target_contract()` currently compares decomposed fields because that is the only source of truth
- `README.md` and `readme-runner-config.yml` currently show only the verbose decomposed destination style.
- The runner already has real runtime coverage that can prove destination connectivity:
  - `bootstrap_contract.rs` writes runner configs and exercises real PostgreSQL connection/bootstrap behavior
  - this is the smallest honest runtime slice to prove URL-based destinations work end-to-end
- The local `sqlx-postgres` source confirms:
  - URL parsing already supports the required TLS query parameters
  - getters exist for host, port, database, username, and SSL mode
  - canonical lossy URL rendering exists for normalized comparison

## Boundary Decision

- Keep one explicit destination concept: `PostgresTargetConfig`.
- Refactor `PostgresTargetConfig` so it owns one canonical PostgreSQL connection contract instead of decomposed YAML fields only.
- Preferred internal shape:
  - one stored `PgConnectOptions`
  - lightweight derived fields or methods for:
    - host
    - port
    - database
    - endpoint label
    - target-contract comparison
- Parsing responsibility split:
  - `parser.rs` decides which public YAML shape is being used
  - `PostgresTargetConfig` owns normalization from either:
    - decomposed fields
    - URL string
- Mixed-shape configs must be rejected.
  - do not invent precedence rules
  - greenfield config should fail loudly when both shapes are provided
- Keep the public YAML surface honest:
  - URL mode is `destination.url`
  - decomposed mode remains the existing explicit fields
  - no generic `connection:` wrapper
- Do not expand the runtime destination contract beyond the current explicit PostgreSQL TCP-style target.
  - if URL parsing resolves to a Unix-socket-only shape that cannot be labeled and grouped honestly inside the runner, reject it instead of adding a second destination model implicitly

## Improve-Code-Boundaries Focus

- Primary smell to flatten:
  - the config model currently stores a decomposed YAML shape as if it were the runtime connection contract
- Required cleanup during execution:
  - move destination normalization behind `PostgresTargetConfig`
  - keep `parser.rs` as a thin raw-shape validator instead of a growing conversion bucket
  - stop rebuilding `PgConnectOptions` from scattered fields if one canonical options object can be stored directly
  - compare destination target identity through one normalized contract instead of parallel host/port/user/password/tls fields
- Bold refactor allowance:
  - if `host`, `port`, `user`, `password`, and `tls` fields on `PostgresTargetConfig` become redundant after normalization, remove them
  - if decomposed validation logic wants its own constructor/helper, move it out of the raw serde struct instead of keeping two giant `validate()` branches inline

## Error Contract Decision

- The current `RunnerConfigError::InvalidField` only carries static messages.
- URL parse failures need a clearer dynamic error surface than a generic static string.
- Preferred execution direction:
  - add a config error variant for invalid destination URLs or dynamic invalid-field detail
  - keep the field path explicit as `mappings.destination.url`
  - preserve existing static invalid-field behavior for the decomposed path
- Goal:
  - malformed URLs fail with actionable stderr from `validate-config`
  - do not swallow or flatten the real parse reason

## Intended Files And Structure To Add Or Change

- `crates/runner/src/config/mod.rs`
  - refactor `PostgresTargetConfig` to own the canonical normalized PostgreSQL destination contract
  - keep endpoint/database accessors aligned with current runtime consumers
- `crates/runner/src/config/parser.rs`
  - add `url: Option<String>` to the raw destination shape
  - route URL and decomposed parsing through explicit constructors/helpers
  - reject mixed URL plus decomposed field usage
- `crates/runner/src/error.rs`
  - add the dynamic config error surface needed for clear malformed-URL failures
- `crates/runner/tests/config_contract.rs`
  - add URL-acceptance coverage
  - add malformed-URL rejection coverage
  - add mixed-shape rejection coverage
- `crates/runner/tests/bootstrap_contract.rs`
  - add one runtime integration slice that writes a URL-based destination config and proves the runner connects/bootstrap succeeds
- `README.md`
  - show the URL form first as the concise recommended option
  - keep the decomposed form documented as the explicit alternative
- `crates/runner/tests/fixtures/readme-runner-config.yml`
  - align the README-owned runner config example with the new recommended URL shape
- Optional only if execution proves necessary:
  - `crates/runner/tests/support/runner_public_contract.rs`
    - only adjust if current public-contract assertions become stale after the boundary refactor

## Public Contract Decisions

- Supported destination YAML shapes:

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt
```

or:

```yaml
destination:
  host: pg-a.example.internal
  port: 5432
  database: app_a
  user: migration_user_a
  password: runner-secret-a
  tls:
    mode: verify-ca
    ca_cert_path: /config/certs/destination-ca.crt
```

- Unsupported mixed shape:

```yaml
destination:
  url: postgresql://...
  host: pg-a.example.internal
```

- Mixed shape must fail instead of choosing precedence.
- URL TLS query parameters are the only TLS surface inside URL mode.
  - do not also allow `destination.tls` beside `destination.url`
  - that would create two competing TLS sources of truth

## Vertical TDD Slices

### Slice 1: Tracer Bullet For URL-Based Validate-Config Success

- RED:
  - add one failing `config_contract.rs` test that writes a minimal runner config using only `destination.url`
  - require `validate-config` to succeed
- GREEN:
  - add the smallest parser/model change needed to accept URL-only destination configs
- REFACTOR:
  - move URL normalization into `PostgresTargetConfig` instead of expanding parser branching inline

### Slice 2: Reject Mixed URL And Decomposed Fields

- RED:
  - add one failing contract test that specifies `destination.url` plus any decomposed destination field
  - require a clear validation failure
- GREEN:
  - implement explicit mixed-shape detection
- REFACTOR:
  - keep input-shape classification in one place so the parser does not accumulate scattered “if url.is_some()” checks

### Slice 3: Malformed URL Errors Are Actionable

- RED:
  - add one failing contract test for a malformed PostgreSQL URL
  - require stderr to name `mappings.destination.url` and preserve the real parse reason
- GREEN:
  - add the dynamic config error variant or equivalent rendering needed for a clear failure
- REFACTOR:
  - keep URL-parse failure reporting behind one typed error boundary instead of ad hoc strings

### Slice 4: Canonical Destination Boundary Refactor

- RED:
  - add or adjust the smallest internal/public-facing coverage needed to prove decomposed and URL destinations normalize to the same runtime target semantics
  - specifically protect destination grouping/target comparison behavior
- GREEN:
  - refactor `PostgresTargetConfig` to own canonical `PgConnectOptions`
  - derive endpoint labels and contract comparison from the normalized target
- REFACTOR:
  - remove redundant fields or reconstruction code once one source of truth exists

### Slice 5: Real Runtime Connectivity With URL-Based Destinations

- RED:
  - add one failing `bootstrap_contract.rs` integration that writes a URL-based destination config against the existing test Postgres harness
  - require bootstrap/runtime startup to connect successfully
- GREEN:
  - fix any runtime-path assumptions that still depend on the decomposed config shape
- REFACTOR:
  - keep config-writing helpers honest; if a helper exists only to spell out decomposed fields, replace or generalize it instead of duplicating a second URL-only helper maze

### Slice 6: README And README-Owned Fixture Contract

- RED:
  - add failing README/operator-surface assertions if needed so the recommended runner config example uses `destination.url`
  - keep coverage that the decomposed shape still appears as the explicit alternative if documented
- GREEN:
  - update `README.md` and `readme-runner-config.yml`
- REFACTOR:
  - remove stale docs that imply decomposed destination fields are the only supported runner path

### Slice 7: Final Lanes And Boundary Pass

- RED:
  - after the behavior slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long`
- GREEN:
  - continue until every required default lane passes
- REFACTOR:
  - do one final `improve-code-boundaries` pass and confirm the destination config boundary is smaller and more honest than before

## TDD Guardrails For Execution

- One failing behavior slice at a time.
- Do not add tests after implementation for the same behavior.
- Test through public runner surfaces first:
  - `validate-config`
  - real bootstrap/runtime connectivity
  - README-owned operator docs
- Do not allow two competing TLS sources of truth for URL mode.
- Do not silently fall back from URL mode to decomposed mode.
- Do not swallow `sqlx` parse failures behind vague static messages.
- If execution discovers that URL mode requires a second destination type instead of a deeper single module, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Final Verification For The Execution Turn

- [x] Red/green TDD covers URL-based validate-config success, malformed URL failure, and mixed-shape rejection
- [x] `PostgresTargetConfig` owns one canonical normalized PostgreSQL destination contract instead of mirroring only one YAML shape
- [x] Runtime bootstrap/connectivity is proven to work with a URL-based destination config
- [x] README documents the URL shape as the concise recommended operator path while preserving the decomposed alternative
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` not run because this task does not require the long lane
- [x] Final `improve-code-boundaries` pass confirms the config boundary got simpler rather than muddier
- [x] Update the task file and set `<passes>true</passes>` only after all required lanes pass

Plan path: `.ralph/tasks/story-01-runner-config-ergonomics/task-02-runner-destination-connection-string_plans/2026-04-25-runner-destination-connection-string-plan.md`

NOW EXECUTE
