# Plan: Compare Cockroach And PostgreSQL Schema Exports Semantically

## References

- Task: `.ralph/tasks/story-05-schema-validation/01-task-compare-schema-exports-semantically.md`
- Next task: `.ralph/tasks/story-05-schema-validation/02-task-generate-helper-shadow-ddl-and-dependency-order.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Design: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Investigation: `designs/crdb-to-postgres-cdc/01_investigation_log.md`
- Investigation artifacts:
  - `investigations/cockroach-webhook-cdc/output/schema-compare/crdb_schema.txt`
  - `investigations/cockroach-webhook-cdc/output/schema-compare/pg_schema.sql`
  - `investigations/cockroach-webhook-cdc/output/schema-compare/raw_diff.txt`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumption

- The task markdown plus the design and investigation docs above are treated as approval for the command shape, semantic comparison scope, and test matrix in this plan.
- If the first execution slices prove that the selected CLI surface is the wrong public boundary, or that the validated schema model cannot survive task 02 without duplicate parsing or duplicate type shapes, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Add one offline `runner compare-schema` command rather than pushing schema-export paths into the long-running runtime config.
- The command contract should be:
  - `runner compare-schema --config <path> --mapping <id> --cockroach-schema <path> --postgres-schema <path>`
- Use `--mapping <id>` plus the existing config mapping table list as the table-selection contract. That is how excluded tables are supported cleanly:
  - selected mapping tables are compared
  - tables present only in the exports but not selected for that mapping are ignored
- Keep schema comparison completely offline:
  - no `sqlx`
  - no live PostgreSQL connections
  - no Cockroach connections
  - no `run` startup coupling in this story
- Introduce one canonical typed schema boundary that both export parsers target and that task 02 can reuse directly:
  - `ValidatedSchema`
  - `TableSchema`
  - `ColumnSchema`
  - `PrimaryKeyShape`
  - `ForeignKeyShape`
  - `UniqueConstraintShape`
  - `IndexShape`
- Apply `improve-code-boundaries` aggressively:
  - move schema export parsing and semantic comparison out of `lib.rs`
  - keep parser-specific cleanup at the parser edge only
  - compare only canonical typed schema, never raw lines or ad-hoc string buckets
  - remove the duplicated SQL identifier and qualified-table-name shapes from `postgres_setup.rs` and `postgres_bootstrap.rs` by introducing one shared identifier module that the new schema code also uses
- The compare command should return either:
  - one validated schema summary that task 02 can build on
  - or a typed mismatch report with actionable per-table diagnostics

## Semantic Compatibility Contract

- The comparison is semantic, not textual. These export-only differences must not fail the check:
  - file headers and `SET` statements
  - PostgreSQL dump comments and `\restrict` / `\unrestrict`
  - Cockroach `WITH (schema_locked = true)`
  - Cockroach `VALIDATE CONSTRAINT` lines
  - constraint and index naming differences when the structural definition is the same
- Compare only the selected tables for the chosen mapping.
- Table order in the export files must not matter.
- Column order should be preserved in the typed schema for later helper-DDL generation, but comparison should match columns by name rather than fail on reordered declarations.
- The first supported cross-dialect type families should be explicit and small:
  - Cockroach `STRING`, `VARCHAR`, and PostgreSQL `character varying(...)` / `varchar(...)` / `text` normalize to one string family
  - Cockroach `INT`, `INT8`, `BIGINT` and PostgreSQL `integer` / `bigint` normalize to one signed-integer family
  - Cockroach `BOOL` and PostgreSQL `boolean` normalize to one boolean family
  - Cockroach `TIMESTAMPTZ` and PostgreSQL `timestamp with time zone` normalize to one timestamptz family
- The reason for the relaxed string-family rule is grounded in the investigation artifacts:
  - the Cockroach export collapses string declarations to `STRING`
  - the PostgreSQL export preserves `varchar(n)`
  - raw equality would incorrectly reject the investigated matching schema
- For this story, any unsupported or ambiguous type pair must fail loudly with a typed mismatch instead of silently guessing.
- The semantic comparison must cover the in-scope structure from the task:
  - table presence
  - columns
  - normalized type family
  - nullability
  - primary keys
  - foreign keys including referenced table, referenced columns, and `ON DELETE` action
  - unique constraints
  - relevant non-unique index structure including column order and sort direction

## Public Contract To Establish

- `runner compare-schema --config <path> --mapping <id> --cockroach-schema <path> --postgres-schema <path>` exits successfully when the selected tables are semantically compatible.
- On success, stdout should give a short summary that is usable in scripts and by a human:
  - mapping id
  - number of compared tables
  - number of ignored tables
- On mismatch, the command must exit non-zero and print actionable diagnostics such as:
  - missing selected table on one side
  - missing column
  - incompatible normalized type family
  - nullability mismatch
  - missing or incompatible PK, FK, unique constraint, or index structure
- The command must not produce a raw unified diff as its user-facing output.
- The validated schema model retained after a successful compare must contain enough information for task 02 to generate helper shadow DDL and dependency order without reparsing or introducing parallel schema DTOs.

## Files And Structure To Add Or Change

- [x] `crates/runner/src/lib.rs`
  - add the `compare-schema` subcommand and keep command dispatch thin
- [x] `crates/runner/src/config/mod.rs`
  - add a narrow mapping lookup helper for `--mapping <id>` without leaking config internals into the compare module
- [x] `crates/runner/src/error.rs`
  - add typed parse, selection, and schema-mismatch error boundaries for the compare command
- [x] `crates/runner/src/sql_name.rs`
  - new shared SQL identifier and qualified-table-name types reused by bootstrap, artifact rendering, and schema comparison
- [x] `crates/runner/src/schema_compare/mod.rs`
  - new compare entry point and typed validated-schema model
- [x] `crates/runner/src/schema_compare/cockroach_export.rs`
  - Cockroach export parser and normalization boundary
- [x] `crates/runner/src/schema_compare/postgres_export.rs`
  - PostgreSQL schema dump parser and normalization boundary
- [x] `crates/runner/src/schema_compare/report.rs`
  - typed mismatch reporting and human-readable rendering
- [x] `crates/runner/tests/schema_compare_contract.rs`
  - new public-interface tests for matching, mismatch, and excluded-table behavior
- [x] `crates/runner/tests/cli_contract.rs`
  - update help assertions for the new subcommand
- [x] `crates/runner/tests/schema_compare_contract.rs` inline temp-file fixtures
  - keep the schema fixtures inside the contract test so each slice declares only the DDL it needs
- [x] `README.md`
  - document the new pre-transfer schema validation command before the runtime starts

## TDD Execution Order

### Slice 1: Tracer Bullet For Semantic Match

- [x] RED: add one CLI contract test that runs `runner compare-schema` against the investigated matching export pair for one mapping and fails because the command does not exist yet
- [x] GREEN: add the new subcommand, load the selected mapping from config, parse the two export files just enough to prove the investigated schema passes, and print a success summary
- [x] REFACTOR: extract the shared identifier and qualified-table-name module so schema parsing does not clone the same shapes already living in bootstrap and artifact code

### Slice 2: Mismatch Reporting

- [x] RED: add one failing contract test for a selected-table mismatch such as a missing column or missing table
- [x] GREEN: implement typed mismatch reporting with a stable human-readable message and non-zero exit status
- [x] REFACTOR: keep mismatch collection in a dedicated report type instead of sprinkling `format!` calls through the parser and command layer

### Slice 3: Excluded Tables Through Mapping Selection

- [x] RED: add a failing test where the export files contain extra tables that are not listed under the chosen mapping, and assert that the compare still succeeds while reporting ignored-table counts
- [x] GREEN: use the mapping table list as the canonical selection filter on both parsed catalogs
- [x] REFACTOR: centralize table selection once so Cockroach and PostgreSQL paths cannot drift on excluded-table logic

### Slice 4: Constraint And Index Structure

- [x] RED: add failing tests for FK mismatch, unique-constraint mismatch, and non-unique index mismatch while proving that name-only constraint differences do not fail the compare
- [x] GREEN: compare PK, FK, unique, and index structure using canonical typed shapes rather than raw DDL fragments
- [x] REFACTOR: keep constraint canonicalization in the schema module so later helper-DDL generation reuses the same structural facts

### Slice 5: Typed Normalization And Unsupported Types

- [x] RED: add failing tests for one supported cross-dialect type family and one unsupported pair
- [x] GREEN: implement the explicit normalization table and fail loudly for unsupported or ambiguous pairs
- [x] REFACTOR: keep type-family normalization in one module-local function or enum impl so later tasks do not recreate compatibility tables

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass and remove any leftover duplicate identifier types, parser-leaks into `lib.rs`, or string-bucket mismatch code

## Boundary Review Checklist

- [x] No schema export paths are added to the long-running runner YAML config
- [x] No live database connectivity is added to the compare command
- [x] No raw text diff or line-by-line string comparison survives in the user-facing comparator
- [x] No duplicate SQL identifier or qualified-table-name types remain across runner modules
- [x] No mismatch is swallowed or downgraded to an opaque string bucket
- [x] No parser-specific cleanup logic leaks into `lib.rs`
- [x] The validated schema model is reusable by task 02 without reparsing or duplicate DTOs

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
