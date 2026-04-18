# Plan: PostgreSQL Helper Bootstrap From The Runner

## References

- Task: `.ralph/tasks/story-04-source-bootstrap/02-task-apply-postgresql-helper-bootstrap-automatically.md`
- Prior task: `.ralph/tasks/story-03-operator-ux-config/02-task-generate-postgresql-grant-sql-and-operator-artifacts.md`
- Design: `designs/crdb-to-postgres-cdc/02_requirements.md`
- Design: `designs/crdb-to-postgres-cdc/03_shadow_table_architecture.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/05_design_decisions.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Design: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Investigation: `designs/crdb-to-postgres-cdc/01_investigation_log.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumption

- The task markdown plus the selected design and investigation docs are treated as approval for this public contract and test surface.
- If the first execution slices show that `runner run` is the wrong public entry point, or that the chosen shadow-table bootstrap boundary cannot survive the later schema-validation stories, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Keep `runner render-postgres-setup --config <path> --output-dir <dir>` as the manual-grants artifact command only. It must not start executing helper bootstrap DDL.
- Make `runner run --config <path>` the automatic destination-side bootstrap entry point. Before it reports readiness, it must connect to every configured destination database and ensure helper state exists.
- Split the current `crates/runner/src/postgres_setup.rs` responsibility in two:
  - offline artifact rendering for grants and operator docs
  - live PostgreSQL bootstrap for helper schema, tracking tables, helper shadow tables, and automatic PK indexing
- Apply `improve-code-boundaries` aggressively:
  - remove `sqlx` connection construction from the config layer
  - keep config as validated data only
  - move all live connection, catalog introspection, and DDL execution into one bootstrap boundary
  - do not let `lib.rs` assemble SQL, inspect catalogs, or decide helper-table names
- Introduce one typed bootstrap plan per mapping, built from validated config plus destination table shape, so the runtime owns one canonical source of truth for:
  - helper schema and tracking-table DDL
  - helper shadow-table naming
  - automatic PK-index decisions
- Keep the shadow-table shape boundary future-proof for story 05:
  - execution for this task may derive table shape from PostgreSQL catalogs
  - later schema-validation work should be able to replace the shape source without rewriting the bootstrap executor
- Bootstrap must be repeatable and idempotent. The runner must either leave PostgreSQL in the required state or fail loudly; no swallowed errors and no best-effort partial success messaging.
- Use one deterministic helper-table naming function that includes mapping identity plus table identity so two mappings cannot collide inside `_cockroach_migration_tool`.
- Define the automatic PK-index rule now: when a mapped real table has primary-key columns, the runner automatically creates one minimal helper index over those columns. There is no operator toggle.

## Public Contract To Establish

- `runner run --config <path>` validates config, bootstraps helper PostgreSQL state, and only then reports `runner ready`.
- The startup path fails loudly if any destination database connection, catalog read, or helper bootstrap step fails.
- Running `runner run --config <path>` twice against the same prepared destination databases succeeds without duplicate-object failures and without drift.
- Bootstrap creates helper schema `_cockroach_migration_tool` inside each destination database, not in a separate control database.
- Bootstrap creates the minimal tracking tables inside `_cockroach_migration_tool`:
  - `stream_state`
  - `table_sync_state`
- Bootstrap prepares one helper shadow table per mapped table using the destination table as the shape source for this story, but without copying serving-oriented structure such as FKs or secondary indexes.
- Helper shadow tables keep data columns and generated/default semantics needed for ingest, but they do not keep serving constraints beyond the optional automatic minimal PK index.
- The minimal PK index decision is automatic and reproducible. Operators do not manage helper indexes manually.
- The grant-artifact contract remains separate:
  - operators still render and apply grants manually
  - once grants exist, starting the runner performs the helper bootstrap automatically

## Bootstrap Shape Decisions

- Create `_cockroach_migration_tool` with `CREATE SCHEMA IF NOT EXISTS`.
- Create `_cockroach_migration_tool.stream_state` as one row per mapping with fields for:
  - mapping id
  - source database
  - source job id
  - starting cursor
  - latest received resolved watermark
  - latest reconciled resolved watermark
  - stream status
- Create `_cockroach_migration_tool.table_sync_state` as one row per mapped table with fields for:
  - mapping id
  - source table identity
  - helper table identity
  - last successful sync time
  - last successful sync watermark
  - last error
- Prepare helper shadow tables through one typed table-shape boundary:
  - inspect the destination table columns and PK columns from PostgreSQL catalogs
  - render one helper table in `_cockroach_migration_tool`
  - keep only the column shape needed for ingest
  - add the automatic minimal PK index when the real table has a PK
- The bootstrap executor, not the config layer, owns:
  - quoting for schema, table, and column identifiers
  - connection setup
  - catalog queries
  - idempotent DDL execution

## Test Strategy For Execution

- Replace the current fake-host `run` contract coverage with real PostgreSQL-backed integration coverage. Once `run` bootstraps live databases, a summary-only fake-config test is no longer a valid public-interface test.
- Use real PostgreSQL for the bootstrap tests:
  - default test lane: add a real-db integration harness for `runner run`
  - long lane: update the Docker-based test path so the `runner` container can actually reach PostgreSQL and still prove the single-binary image contract
- Keep TDD vertical. Each slice adds one new behavior through the public CLI or a real-db integration path, then implements only enough runtime/bootstrap logic to pass.

## Files And Structure To Add Or Change

- [x] `crates/runner/Cargo.toml`
  - enable the runtime/test dependencies actually needed for live PostgreSQL bootstrap and real-db tests
- [x] `crates/runner/src/lib.rs`
  - keep command dispatch only; `run` should invoke bootstrap before rendering the startup summary
- [x] `crates/runner/src/config/mod.rs`
  - remove live `sqlx` connection construction from the config boundary and expose only validated data needed by bootstrap planning
- [x] `crates/runner/src/error.rs`
  - add explicit connect, catalog-introspection, bootstrap-DDL, and readiness errors
- [x] `crates/runner/src/postgres_setup.rs`
  - narrow this to artifact rendering only, or replace it with a `postgres_setup/` module split that keeps artifact code separate from live bootstrap code
- [x] `crates/runner/src/postgres_bootstrap.rs`
  - new typed bootstrap executor and bootstrap report boundary
- [x] `crates/runner/tests/bootstrap_contract.rs`
  - new real-db integration tests for helper bootstrap, repeatability, and helper-table preparation
- [x] `crates/runner/tests/config_contract.rs`
  - remove or replace the fake summary-only `run` assertions that are no longer the correct public contract
- [x] `crates/runner/tests/long_lane.rs`
  - update the container contract to include reachable PostgreSQL and successful bootstrap-on-run behavior
- [x] `crates/runner/tests/fixtures/*.yml`
  - add or adjust runtime fixtures so the real-db tests and long-lane container test have a valid PostgreSQL target
- [x] `README.md`
  - update the quick start so it states clearly that grants are manual but helper bootstrap happens automatically when `runner run` starts

## TDD Execution Order

### Slice 1: Tracer Bullet For Automatic Helper Bootstrap

- [x] RED: add one real PostgreSQL-backed integration test that runs `runner run --config <fixture>` and fails because `_cockroach_migration_tool`, `stream_state`, and `table_sync_state` do not exist yet
- [x] GREEN: implement the minimal startup bootstrap path that connects to one destination database and creates the helper schema plus tracking tables before printing `runner ready`
- [x] REFACTOR: split offline artifact rendering from live bootstrap execution so `postgres_setup` does not become a mixed filesystem-plus-database grab-bag

### Slice 2: Repeatability And Multi-Mapping Coverage

- [x] RED: extend the integration contract to prove that rerunning `runner run` is safe and that multiple mappings bootstrap their own destination databases
- [x] GREEN: make the bootstrap executor idempotent per mapping and produce a typed bootstrap report used by the startup summary
- [x] REFACTOR: centralize per-mapping bootstrap orchestration in one runtime module instead of scattering loops and counters through `lib.rs`

### Slice 3: Helper Shadow Table Preparation

- [x] RED: add one failing real-db test proving that a mapped destination table causes a helper shadow table to be created with matching data columns inside `_cockroach_migration_tool`
- [x] GREEN: inspect the real destination table shape and create the helper shadow table through the bootstrap boundary
- [x] REFACTOR: introduce one typed table-shape model so catalog rows, helper-table names, and DDL rendering do not leak through the command layer

### Slice 4: Automatic Minimal PK Indexing

- [x] RED: add one failing test showing that a mapped table with a primary key gets one automatic helper PK index and that operators are not asked to manage it
- [x] GREEN: inspect PK columns from PostgreSQL catalogs and create the minimal helper index when the rule applies
- [x] REFACTOR: keep the PK-index decision and identifier rendering in the bootstrap module, not in config parsing or test-only helpers

### Slice 5: Loud Failure Modes

- [x] RED: add one failing integration test for a missing mapped table, permission error, or bootstrap catalog mismatch and assert that `runner run` exits with an explicit bootstrap error
- [x] GREEN: add typed bootstrap errors for connect, missing-table, catalog-read, and DDL-execution failures
- [x] REFACTOR: make every bootstrap SQL call go through one narrow error boundary so no failure is swallowed or downgraded to a vague string

### Slice 6: Operator Docs And Docker Contract

- [x] RED: add a failing README or artifact-doc assertion plus a failing long-lane container test that requires `runner run` to bootstrap successfully against reachable PostgreSQL
- [x] GREEN: update docs, fixtures, and Docker-based tests to match the new automatic-bootstrap startup contract
- [x] REFACTOR: remove stale wording that still implies helper bootstrap is a future promise rather than real startup behavior

### Slice 7: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass and remove any leftover config-owned connection helpers, mixed artifact/bootstrap modules, or dead summary-only run code

## Boundary Review Checklist

- [x] No live PostgreSQL bootstrap logic is mixed into the offline grant-artifact renderer
- [x] No `sqlx` connection setup remains in the validated config layer
- [x] No helper-table naming, catalog introspection, or DDL rendering leaks into `lib.rs`
- [x] No operator-managed helper-index toggle is introduced
- [x] No filesystem, connection, catalog, or DDL errors are swallowed
- [x] No fake-host summary-only `run` tests survive once `run` owns real bootstrap behavior
- [x] No separate control database or out-of-band helper bootstrap script is introduced

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
