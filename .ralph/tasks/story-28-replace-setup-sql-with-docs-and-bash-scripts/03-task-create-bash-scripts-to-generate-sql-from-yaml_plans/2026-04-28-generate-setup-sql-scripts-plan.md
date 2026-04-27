# Plan: Generate Canonical Setup SQL Files From YAML With Thin Bash Entrypoints

## References

- Task:
  - `.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/03-task-create-bash-scripts-to-generate-sql-from-yaml.md`
- Canonical SQL contract from Task 02:
  - `docs/setup_sql/index.md`
  - `docs/setup_sql/cockroachdb-source-setup.md`
  - `docs/setup_sql/postgresql-destination-grants.md`
- Source-side sink and changefeed contract:
  - `crates/ingest-contract/src/lib.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/multi_mapping_harness.rs`
- Destination-side privilege contract:
  - `crates/runner/src/postgres_bootstrap.rs`
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
- Test harness dependencies already available in the workspace:
  - `crates/runner/Cargo.toml`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This is the planning turn because the task file had no linked plan artifact yet.
- The task markdown is sufficient approval for this planning turn; the next turn should execute the plan directly unless a contract mismatch forces this file back to `TO BE VERIFIED`.
- This is code work, so the full `tdd` workflow applies:
  - add one failing public-interface test at a time
  - make the minimal script change to pass it
  - refactor only after the slice is green
- Public behavior to test is the CLI contract of the scripts themselves:
  - generated file names
  - generated SQL contents
  - `--help`
  - `--dry-run`
  - invalid-input failures
- `make test` only runs workspace Rust tests plus the Go verifyservice lane, so script contract tests should live in a Rust integration test that shells out to the new scripts.
- If execution shows the SQL contract in `docs/setup_sql/` is wrong or incomplete for the real runtime behavior, switch this plan back to `TO BE VERIFIED` immediately and stop.
- If execution shows the task requires a non-bash implementation or a third script entrypoint, switch this plan back to `TO BE VERIFIED` immediately and stop.

## Current State Summary

- `docs/setup_sql/` exists and is the human-readable contract for the SQL the scripts must render.
- There is no top-level `scripts/` directory yet, so this task creates the entire operator-facing script boundary from scratch.
- The current CockroachDB contract is:
  - enable rangefeeds once with `SET CLUSTER SETTING kv.rangefeed.enabled = true;`
  - capture a cursor once per source database
  - create one changefeed per mapping with:
    - fully qualified table names
    - sink URL ending in `/ingest/<mapping_id>`
    - `initial_scan = 'yes'`
    - `envelope = 'enriched'`
    - `enriched_properties = 'source'`
    - `resolved = '<interval>'`
- The current PostgreSQL contract is:
  - `GRANT CONNECT, CREATE ON DATABASE ...`
  - `GRANT USAGE ON SCHEMA ...`
  - `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE ...`
  - duplicate grants should be omitted when multiple mappings collapse onto the same destination database, schema, role, and table
- The sink URL boundary matters:
  - runtime code composes `/ingest/<mapping_id>` onto a normal HTTPS base URL
  - Cockroach SQL must embed that target as `webhook-https://...`
  - the YAML example uses `webhook.base_url: "https://runner.example.internal:8443"`
  - the scripts therefore should treat `webhook.base_url` as a real HTTPS base URL, trim only trailing slashes, and prepend `webhook-` when rendering the sink

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - two separate operator entrypoints are required, but most of the dangerous logic is shared:
    - CLI parsing
    - dependency checks
    - temp/work file handling
    - dry-run behavior
    - output-dir writes
    - combined-file assembly
    - readable error handling
- Execution should avoid copying that infrastructure into both scripts.
- Planned boundary shape:
  - public entrypoints:
    - `scripts/generate-cockroach-setup-sql.sh`
    - `scripts/generate-postgres-grants-sql.sh`
  - one internal shared shell helper:
    - `scripts/lib/setup_sql_common.sh`
- The shared helper should own generic mechanics only:
  - `--help`, `--dry-run`, `--output-dir`
  - dependency validation
  - temporary output staging
  - file emission and combined-file concatenation
  - consistent `die`/usage/error messaging
- Each public script should own only its domain contract:
  - CockroachDB YAML validation, CA-cert encoding, multi-mapping merge, and changefeed SQL rendering
  - PostgreSQL YAML validation, per-database role/schema/table grouping, deduplication, and grant SQL rendering
- This is the intended boundary cleanup:
  - separate operator workflows stay explicit
  - duplicated shell plumbing does not spread across two scripts
  - SQL rendering remains close to the docs contract instead of being hidden behind a generic template engine

## Public Verification Strategy

- Add an integration test file under `crates/runner/tests/` that invokes the scripts via `std::process::Command`.
- Use fixture YAML inputs and exact expected SQL fixtures because rendered SQL files are the public product of these scripts.
- Avoid testing shell internals or implementation details; verify behavior through the CLI only.
- Required behaviors to cover in red-green order:
  - `generate-cockroach-setup-sql.sh` renders one database file plus `cockroach-all-setup.sql` for a simple config
  - the Cockroach generator merges multiple mappings targeting the same source database into one per-database file while still emitting one `CREATE CHANGEFEED` block per mapping
  - `generate-postgres-grants-sql.sh` renders one database file plus `postgres-all-grants.sql`
  - the Postgres generator deduplicates repeated schema/table grant lines
  - `--dry-run` prints planned outputs and writes no files
  - missing required YAML keys fail with readable stderr and non-zero exit status
  - `--help` prints usage successfully
- Use real CA-cert fixture bytes in tests so the percent-encoded `ca_cert=` query value is proven, not guessed.
- Final repo validation remains:
  - `make check`
  - `make lint`
  - `make test`

## Intended Files To Change

- Create:
  - `scripts/generate-cockroach-setup-sql.sh`
  - `scripts/generate-postgres-grants-sql.sh`
  - `scripts/lib/setup_sql_common.sh`
  - `scripts/README.md`
  - `crates/runner/tests/setup_sql_script_contract.rs`
  - `crates/runner/tests/fixtures/setup_sql_scripts/cockroach-setup-config.yml`
  - `crates/runner/tests/fixtures/setup_sql_scripts/postgres-grants-config.yml`
  - `crates/runner/tests/fixtures/setup_sql_scripts/invalid-cockroach-config.yml`
  - `crates/runner/tests/fixtures/setup_sql_scripts/invalid-postgres-config.yml`
  - expected SQL fixtures for per-database and combined outputs
- Update:
  - `.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/03-task-create-bash-scripts-to-generate-sql-from-yaml.md`
    - add the linked plan path
    - leave acceptance boxes untouched until execution

## Execution Slices

### Slice 1: Tracer Bullet For Cockroach Happy Path

- RED:
  - add a Rust integration test that runs `scripts/generate-cockroach-setup-sql.sh` with a minimal valid YAML fixture and a real CA cert fixture
  - assert:
    - exit status is success
    - one per-database file is written
    - `cockroach-all-setup.sql` is written
    - file contents exactly match expected SQL fixtures
- GREEN:
  - create `scripts/` structure
  - add `setup_sql_common.sh` with strict shell options, usage helpers, dependency detection, output-dir creation, and shared emit helpers
  - implement the Cockroach script with the minimum YAML parsing and SQL rendering needed to pass this first test
- Refactor target after green:
  - keep SQL rendering obvious and line-oriented, not spread across many small shell functions

### Slice 2: Cockroach Multi-Mapping Merge And CLI Surface

- RED:
  - extend the contract tests for:
    - two mappings targeting the same source database
    - `--dry-run` producing preview output without writing files
    - `--help` succeeding with usage text
    - invalid/missing Cockroach YAML keys failing clearly
- GREEN:
  - implement grouping by source database
  - keep one `CREATE CHANGEFEED` statement per mapping block
  - emit one shared header and cursor guidance per source database file
  - ensure the combined file concatenates the per-database rendered blocks deterministically
  - validate required keys:
    - `cockroach.url`
    - `webhook.base_url`
    - `webhook.ca_cert_path`
    - `webhook.resolved`
    - each mapping `id`
    - each mapping `source.database`
    - each mapping `source.tables`
- Refactor target after green:
  - any YAML-query helpers that are general should move into the shared shell library

### Slice 3: Postgres Tracer Bullet And Dedup Contract

- RED:
  - add a Rust integration test that runs `scripts/generate-postgres-grants-sql.sh` with a valid YAML fixture
  - assert:
    - exit status is success
    - one per-database file and `postgres-all-grants.sql` are written
    - SQL matches expected fixtures exactly
  - then add a failing case where duplicate grants would be produced from overlapping mappings
- GREEN:
  - implement the Postgres script on top of the shared shell helper
  - group by destination database
  - deduplicate identical grant statements per output file while preserving deterministic ordering
  - validate required keys:
    - each mapping `id`
    - each mapping `destination.database`
    - each mapping `destination.runtime_role`
    - each mapping `destination.tables`
- Refactor target after green:
  - keep Postgres-specific grouping local to that script; do not generalize away the domain model

### Slice 4: Docs Alignment, Executable Bits, And Full Validation

- Ensure both scripts:
  - have executable permissions
  - declare dependencies clearly in a header comment
  - prefer `yq` when present, otherwise fall back to `python3`
  - fail loudly if neither YAML parser path is available
- Write `scripts/README.md` with:
  - dependencies
  - input format summary
  - usage examples
  - dry-run examples
  - example output file names
- Run:
  - targeted script contract tests while iterating
  - `make check`
  - `make lint`
  - `make test`
- Final mud check using `improve-code-boundaries`:
  - confirm shared shell plumbing lives in one internal helper instead of two cloned entrypoints
  - confirm the operator-visible scripts remain standalone and explicit
  - confirm the rendered SQL still matches `docs/setup_sql/` exactly rather than inventing a second contract
- If any validation failure shows the docs contract itself is inconsistent, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Done Condition

- Two standalone executable bash entrypoints exist under `scripts/` and generate the documented SQL outputs from YAML.
- Shared non-domain shell plumbing lives in one internal helper rather than being copied.
- Script behavior is covered by public-interface Rust integration tests that invoke the CLIs and compare their output files.
- `scripts/README.md` explains how operators run the tools and what dependencies they need.
- The repo passes `make check`, `make lint`, and `make test`.

Plan path: `.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/03-task-create-bash-scripts-to-generate-sql-from-yaml_plans/2026-04-28-generate-setup-sql-scripts-plan.md`

NOW EXECUTE
