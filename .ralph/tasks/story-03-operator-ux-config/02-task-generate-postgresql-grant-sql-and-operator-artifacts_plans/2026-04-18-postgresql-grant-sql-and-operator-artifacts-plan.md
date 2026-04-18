# Plan: PostgreSQL Grant SQL And Operator Artifacts

## References

- Task: `.ralph/tasks/story-03-operator-ux-config/02-task-generate-postgresql-grant-sql-and-operator-artifacts.md`
- Prior task: `.ralph/tasks/story-03-operator-ux-config/01-task-define-single-config-yaml-and-multi-db-mapping.md`
- Design: `designs/crdb-to-postgres-cdc/01_investigation_log.md`
- Design: `designs/crdb-to-postgres-cdc/03_shadow_table_architecture.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Design: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumption

- The task markdown plus the selected design docs are treated as the approval for this public contract and test surface.
- If the first execution slices show that the output shape or grant contract is wrong, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Keep one operator-facing runner config as the only source of truth. Do not introduce a second grant-specific config file or a duplicated runtime projection layer for artifacts.
- Add one operator-facing runner subcommand dedicated to PostgreSQL preparation output:
  - `runner render-postgres-setup --config <path> --output-dir <dir>`
- Generate deterministic per-mapping artifacts from the validated config:
  - `<output-dir>/<mapping-id>/grants.sql`
  - `<output-dir>/<mapping-id>/README.md`
- Generate one index file for the overall operator flow:
  - `<output-dir>/README.md`
- Keep translation from validated config to grant requirements in one typed planning module. `lib.rs` should dispatch commands, not assemble SQL strings or duplicate grant rules inline.
- Use one canonical `PostgresGrantPlan` or similarly named type per mapping and render both SQL and human-readable guidance from that type. This is the required `improve-code-boundaries` cleanup for this task.
- Keep role grants explicit SQL artifacts only. Do not connect to PostgreSQL and do not execute grant SQL in this task.
- Keep helper bootstrap separate from grant artifacts:
  - this task explains what the runtime role must be able to do
  - story-04/02 later performs the automatic helper-schema and helper-table bootstrap

## Public Contract To Establish

- `runner render-postgres-setup --config <path> --output-dir <dir>` loads the same validated multi-mapping config used by `validate-config` and `run`.
- For each mapping, generated artifacts make the PostgreSQL setup requirements explicit for the configured destination database and runtime role.
- Generated SQL remains scoped:
  - no superuser commands
  - no role creation
  - no blanket cluster-wide grants
  - no `GRANT` statements for tables outside the configured mapping tables
- Generated SQL must cover the minimal permissions implied by the investigation and design:
  - `CONNECT` on the destination database
  - `TEMPORARY` on the destination database
  - `CREATE` on the destination database so the runtime role can later create `_cockroach_migration_tool`
  - `USAGE` on schema `public`
  - `SELECT, INSERT, UPDATE, DELETE` on each mapped destination table
- Generated operator guidance must make the helper schema contract explicit:
  - the runtime later bootstraps `_cockroach_migration_tool` automatically
  - if `_cockroach_migration_tool` already exists, it must be owned by the configured runtime role
  - no superuser requirement is assumed or recommended
- The command prints a short summary describing where the artifacts were written and how many mapping bundles were produced.

## Artifact Shape

### Per-Mapping `grants.sql`

- Header comment naming:
  - mapping id
  - destination database
  - runtime role
  - helper schema `_cockroach_migration_tool`
- Deterministic SQL statements in stable order:
  - `GRANT CONNECT, TEMPORARY, CREATE ON DATABASE ...`
  - `GRANT USAGE ON SCHEMA public TO ...`
  - `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE ... TO ...`
- Use identifier quoting where needed so unusual but valid names stay reproducible.

### Per-Mapping `README.md`

- Explain which SQL file to run and against which database.
- State that grants stay manual and explicit by design.
- State that the runner will later create helper objects automatically after grants are in place.
- State the preexisting-schema contract for `_cockroach_migration_tool`.

### Top-Level `README.md`

- Summarize all mappings and the location of each artifact bundle.
- Describe the minimal operator sequence:
  - render artifacts
  - review and run each `grants.sql`
  - start the runner later

## Files And Structure To Add Or Change

- [x] `crates/runner/src/lib.rs`
  - add the new subcommand and command output path
- [x] `crates/runner/src/config/mod.rs`
  - expose only the behavior needed for artifact planning from validated config
- [x] `crates/runner/src/config/parser.rs`
  - keep validation centralized if artifact generation reveals missing identifier constraints
- [x] `crates/runner/src/postgres_setup.rs`
  - new typed planning and rendering module for grant requirements and operator artifacts
- [x] `crates/runner/src/error.rs`
  - add explicit artifact rendering and file-write errors; do not swallow filesystem failures
- [x] `crates/runner/tests/cli_contract.rs`
  - assert the new subcommand is discoverable
- [x] `crates/runner/tests/config_contract.rs`
  - keep existing config behavior green while adding contract tests for artifact generation
- [x] `crates/runner/tests/fixtures/valid-runner-config.yml`
  - reuse for the new command
- [x] `README.md`
  - refresh the runner quick start after the command contract is real; remove the stale legacy single-`postgres` config example while touching docs

## TDD Execution Order

### Slice 1: Tracer Bullet For Artifact Rendering

- [x] RED: add one integration-style test that runs `runner render-postgres-setup --config <fixture> --output-dir <tmpdir>` and fails because no artifacts are written yet
- [x] GREEN: implement the minimal command path that loads validated config and writes one top-level README plus one mapping bundle
- [x] REFACTOR: move grant planning and file layout decisions out of `lib.rs` into a dedicated planning module

### Slice 2: Scoped Database And Schema Grants

- [x] RED: extend the artifact contract test to require the minimal database-level grants and `public` schema usage grant for each mapping role
- [x] GREEN: render deterministic SQL for `CONNECT`, `TEMPORARY`, `CREATE`, and `USAGE` on `public`
- [x] REFACTOR: keep privilege enumeration typed, not stringly booleans or ad hoc SQL fragments scattered across the CLI layer

### Slice 3: Table-Level DML Grants From Mapping Tables

- [x] RED: add one failing test that proves table grants are derived only from configured mapping tables and remain stable in order
- [x] GREEN: map each configured table into a table grant statement for the configured destination role
- [x] REFACTOR: derive quoted identifiers through one helper owned by the planning module instead of duplicating escaping logic

### Slice 4: Helper Schema Ownership Contract

- [x] RED: add one failing test for the operator-facing README content that explains `_cockroach_migration_tool` ownership and the separation between manual grants now and automatic bootstrap later
- [x] GREEN: render the per-mapping and top-level README files with the explicit helper-schema contract
- [x] REFACTOR: keep human-readable guidance generated from the same typed grant-plan model used for SQL so the docs and SQL cannot drift

### Slice 5: Error Boundary And CLI Surface

- [x] RED: add one failing test for a file-write failure or invalid output directory behavior and one CLI help assertion for the new subcommand
- [x] GREEN: introduce explicit write errors in `RunnerError` and wire the new command into CLI help
- [x] REFACTOR: ensure filesystem writes happen through a narrow boundary that returns explicit errors rather than partial silent output

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass and remove any leftover legacy README config example or duplicate artifact-rendering code

## Boundary Review Checklist

- [x] No second config file or duplicated config projection is introduced for grant generation
- [x] No SQL string assembly for grant artifacts lives in `lib.rs`
- [x] No filesystem errors are ignored during artifact generation
- [x] No superuser-only PostgreSQL assumptions appear in SQL or operator docs
- [x] No grant output includes tables outside the configured mapping list
- [x] No helper-schema bootstrap DDL is mixed into this task beyond documenting the contract for the later automatic bootstrap task
- [x] No stale single-`postgres` README example survives once the docs are touched

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
