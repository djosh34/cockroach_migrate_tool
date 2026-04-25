# Plan: Add Deep Validation To `runner validate-config`

## References

- Task:
  - `.ralph/tasks/story-01-runner-config-ergonomics/task-04-runner-deep-validation.md`
- Current runner CLI and validation output:
  - `crates/runner/src/lib.rs`
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/support/operator_cli_surface.rs`
- Current config and destination target boundary:
  - `crates/runner/src/config/mod.rs`
  - `crates/runner/src/config/parser.rs`
  - `crates/runner/src/runtime_plan.rs`
- Current bootstrap/catalog boundary:
  - `crates/runner/src/postgres_bootstrap.rs`
  - `crates/runner/src/error.rs`
- Current config and runtime contract coverage:
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/bootstrap_contract.rs`
- Current operator docs and fixtures:
  - `README.md`
  - `crates/runner/tests/fixtures/valid-runner-config.yml`
  - `crates/runner/tests/fixtures/readme-runner-config.yml`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient product approval for this planning turn.
- This turn is planning-only because the task had no linked plan artifact yet.
- This is greenfield work.
  - no backward-compatibility shims
  - no hidden fallback from deep to shallow validation
  - no reuse of bootstrap write paths for a read-only command
- The default `validate-config` path must stay offline and fast.
- `validate-config --deep` is allowed to make real PostgreSQL connections to destination targets only.
- If the first RED slice proves that the read-only destination checks cannot be extracted cleanly from bootstrap without inventing a second overlapping schema model, switch this plan back to `TO BE VERIFIED` and stop immediately.
- If the first RED slice proves that the success/error surface needs materially different user-facing wording than this plan assumes, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - `validate-config --deep` connects to each destination
  - `validate-config --deep` fails loudly with mapping ID and endpoint when credentials or connectivity are wrong
  - `validate-config --deep` fails loudly when a mapped table does not exist
  - plain `validate-config` remains offline and does not require a reachable destination
  - both decomposed and URL destinations work because deep validation consumes the normalized `PostgresTargetConfig`
- Lower-priority concerns:
  - emitting a large catalog summary in the success output
  - adding CockroachDB source checks
  - reusing bootstrap-only write scaffolding for convenience

## Current State Summary

- `runner validate-config` currently only loads YAML and emits a `ValidatedConfig` summary.
- The CLI surface has no `--deep` flag today.
- `postgres_bootstrap.rs` already knows how to:
  - connect to a destination with `PgConnection::connect_with`
  - inspect selected destination tables
  - report connection and missing-table failures with mapping/database context
- That boundary is muddy for this task because it mixes:
  - read-only catalog inspection needed by `validate-config --deep`
  - write-side bootstrap DDL and tracking-state seeding needed only by `runner run`
- `config_contract.rs` currently covers CLI behavior and config validation, but it has no shared PostgreSQL harness.
- `bootstrap_contract.rs` already owns a real ephemeral PostgreSQL harness that can likely be extracted into shared test support rather than duplicated.

## Boundary Decision

- Keep one public validation command:
  - `runner validate-config --config <path>`
  - `runner validate-config --config <path> --deep`
- Introduce one explicit read-only destination validation boundary, separate from bootstrap writes.
- Preferred execution direction:
  - extract destination connection and catalog probing into a dedicated module such as `destination_validation` or similarly named runner-internal boundary
  - let `validate-config --deep` call that boundary directly
  - let `postgres_bootstrap.rs` reuse the extracted read-only catalog loader instead of owning it inline
- Do not make `validate-config --deep` construct a `RunnerStartupPlan` if doing so drags in runtime-only concerns.
  - if grouping logic is useful, reuse only the honest destination grouping/data access pieces
- Keep one canonical destination type:
  - `PostgresTargetConfig`
  - no second destination representation for deep validation

## Improve-Code-Boundaries Focus

- Primary smell to flatten:
  - `postgres_bootstrap.rs` currently owns both read-only destination introspection and write-time bootstrap side effects
- Required cleanup during execution:
  - move read-only destination probing behind a dedicated typed boundary
  - keep bootstrap focused on scaffold creation and helper-plan materialization
  - keep `validate-config` focused on command semantics and output rendering, not SQL details
  - avoid stringly `deep` state by introducing a small typed status for validation output
- Bold refactor allowance:
  - if enough read-only schema loading moves out of `postgres_bootstrap.rs`, reduce or delete duplicated helpers there
  - if the PostgreSQL test harness in `bootstrap_contract.rs` must be shared with `config_contract.rs`, extract it into `crates/runner/tests/support` instead of copy-pasting it

## Error And Success Contract Decision

- Deep validation needs its own typed error boundary instead of shoving network/catalog failures into raw config-parse errors.
- Preferred execution direction:
  - add a `RunnerValidateConfigError`-style boundary or equivalent that still renders with a `config:` prefix at the CLI
  - preserve explicit fields in failure messages:
    - mapping ID
    - endpoint label
    - missing table name when relevant
- Success rendering decision:
  - plain `validate-config` keeps the current fast summary shape
  - `--deep` success adds an explicit deep marker such as `deep=ok`
  - JSON log output should expose the deep result in a structured field when deep mode runs
- Failure examples to preserve as contracts:
  - `config: failed to connect mapping \`app-a\` to \`127.0.0.1:5432/app_a\`: ...`
  - `config: missing mapped destination table \`public.customers\` for mapping \`app-a\` in \`app_a\``

## Intended Files And Structure To Add Or Change

- `crates/runner/src/lib.rs`
  - add `--deep` to the `validate-config` subcommand
  - route deep validation through the new read-only boundary
  - extend `ValidatedConfig` output with typed deep status
- `crates/runner/src/error.rs`
  - add the typed error surface for deep validation failures
- `crates/runner/src/postgres_bootstrap.rs`
  - remove or reuse any read-only catalog helpers that belong in the new validation boundary
- `crates/runner/src/runtime_plan.rs`
  - only if needed, expose or extract a smaller destination-grouping seam that deep validation can reuse honestly
- New runner-internal module, name to be decided during RED/GREEN:
  - own destination connection + mapped-table existence checks
  - possibly own reusable destination schema loading for both deep validation and bootstrap
- `crates/runner/tests/config_contract.rs`
  - add deep success coverage
  - add deep missing-table failure coverage
  - add deep offline-default coverage
  - add URL-based deep success or failure coverage
- `crates/runner/tests/cli_contract.rs`
  - update help expectations for `--deep`
- `crates/runner/tests/support/operator_cli_surface.rs`
  - add `--deep` to the validate-config help contract
- `crates/runner/tests/bootstrap_contract.rs`
  - adjust only if catalog-loading extraction changes its helper usage
- `crates/runner/tests/support/...`
  - extract the ephemeral PostgreSQL harness here if `config_contract.rs` needs it
- `README.md`
  - document `runner validate-config --deep`
  - explain that plain validation is offline and deep validation checks destination reachability and table presence

## Public Contract Decisions

- Supported command forms:

```bash
runner validate-config --config /config/runner.yml
runner validate-config --config /config/runner.yml --deep
```

- Success summary expectations:
  - shallow mode keeps the existing summary and remains offline
  - deep mode adds `deep=ok`
- Failure contract expectations:
  - deep validation failures must include mapping ID
  - connectivity failures must include the endpoint label
  - missing-table failures must include the selected table name
- Deep validation scope:
  - connect to destination PostgreSQL targets only
  - verify every mapped table named in `mappings[].source.tables` exists in the destination catalog
  - do not create helper schemas or tables
  - do not write any bootstrap/tracking data

## Vertical TDD Slices

### Slice 1: CLI Surface Exposes `--deep`

- RED:
  - add one failing CLI/help contract assertion that `runner validate-config --help` includes `--deep`
- GREEN:
  - add the new flag in `crates/runner/src/lib.rs`
- REFACTOR:
  - keep the command shape small; no extra subcommands or hidden aliases

### Slice 2: Deep Validation Connects Successfully

- RED:
  - add one failing `config_contract.rs` test that starts a real PostgreSQL destination, writes a valid runner config, runs `validate-config --deep`, and expects success with `deep=ok`
- GREEN:
  - implement the smallest read-only validation path needed to connect and report deep success
- REFACTOR:
  - if the test needs the existing Postgres harness, extract it once into shared support rather than duplicating process management

### Slice 3: Default Validation Stays Offline

- RED:
  - add one failing `config_contract.rs` test that writes a config pointing at an unreachable destination and proves plain `validate-config` still succeeds without `--deep`
- GREEN:
  - ensure the shallow path does not call the deep validator
- REFACTOR:
  - keep deep state explicit in the command flow instead of boolean checks scattered through output rendering

### Slice 4: Missing Tables Fail Loudly

- RED:
  - add one failing `config_contract.rs` test where credentials/connectivity are valid but one mapped table is absent
  - require stderr to name the mapping and missing table
- GREEN:
  - implement read-only catalog existence checks against the destination
- REFACTOR:
  - return a typed table-missing error from the new validation boundary instead of formatting strings at the call site

### Slice 5: URL Destinations Work In Deep Mode

- RED:
  - add one failing `config_contract.rs` deep-validation test using `destination.url`
- GREEN:
  - make deep validation consume the existing normalized `PostgresTargetConfig` only
- REFACTOR:
  - avoid any branch that re-parses destination shape differently for deep validation versus runtime

### Slice 6: Bootstrap Reuses The Read-Only Catalog Boundary

- RED:
  - add or adjust the smallest honest coverage needed to prove bootstrap still loads destination schema correctly after extraction
- GREEN:
  - switch bootstrap to reuse the extracted catalog-loading/probing module
- REFACTOR:
  - delete duplicate read-only helpers left behind in `postgres_bootstrap.rs`

### Slice 7: README And Validation Output Contract

- RED:
  - add or adjust the smallest docs/public-surface coverage needed so README and command contracts mention `--deep`
- GREEN:
  - update README guidance and any owned fixtures/examples
- REFACTOR:
  - remove stale wording that implies config validation never touches a destination under any mode

### Slice 8: Final Lanes And Boundary Pass

- RED:
  - after behavior slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long`
- GREEN:
  - continue until every required default lane passes
- REFACTOR:
  - do one final `improve-code-boundaries` pass and confirm read-only validation no longer lives inside bootstrap writes

## TDD Guardrails For Execution

- One failing behavior slice at a time.
- Do not add tests after implementation for the same behavior.
- Prefer public-surface tests:
  - CLI help
  - `validate-config`
  - README/operator contracts
- Never swallow `sqlx` connection or catalog errors.
- Never let deep validation create schemas, tables, or tracking rows.
- If execution reveals that table existence checks need a materially different contract than `mappings[].source.tables`, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Final Verification For The Execution Turn

- [x] Red/green TDD covers `--deep` CLI exposure, deep success, offline shallow validation, missing-table failure, and URL-based deep validation
- [x] Read-only destination probing is extracted away from bootstrap writes
- [x] Deep validation errors include mapping ID and endpoint context when relevant
- [x] Deep validation proves mapped destination tables exist without mutating the destination
- [x] README documents `validate-config --deep` and the shallow-vs-deep distinction
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` not run because this task does not require the long lane
- [x] Final `improve-code-boundaries` pass confirms the runner boundary is simpler rather than muddier
- [x] Update the task file and set `<passes>true</passes>` only after all required lanes pass

Plan path: `.ralph/tasks/story-01-runner-config-ergonomics/task-04-runner-deep-validation_plans/2026-04-25-runner-deep-validation-plan.md`

NOW EXECUTE
