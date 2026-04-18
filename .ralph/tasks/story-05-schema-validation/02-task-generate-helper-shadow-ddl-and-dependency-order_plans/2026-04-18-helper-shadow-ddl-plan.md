# Plan: Generate Helper Shadow DDL And Dependency Order From The Validated Schema

## References

- Task: `.ralph/tasks/story-05-schema-validation/02-task-generate-helper-shadow-ddl-and-dependency-order.md`
- Previous task plan: `.ralph/tasks/story-05-schema-validation/01-task-compare-schema-exports-semantically_plans/2026-04-18-schema-compare-semantic-plan.md`
- Design: `designs/crdb-to-postgres-cdc/03_shadow_table_architecture.md`
- Design: `designs/crdb-to-postgres-cdc/05_design_decisions.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Investigation: `designs/crdb-to-postgres-cdc/01_investigation_log.md`
- Current implementation: `crates/runner/src/schema_compare/mod.rs`
- Current implementation: `crates/runner/src/postgres_bootstrap.rs`
- Current tests: `crates/runner/tests/bootstrap_contract.rs`
- Current tests: `crates/runner/tests/long_lane.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumption

- The task markdown plus the design package above are treated as approval for two boundaries:
  - one reusable typed schema model that is shared by schema comparison, helper-DDL generation, and bootstrap
  - one helper-plan generator that produces both helper shadow DDL and typed reconcile ordering from that schema model
- This task should establish a public artifact-rendering contract for the generated helper plan instead of hiding the new behavior behind private-only functions.
- If the first execution slices prove that a public `render-helper-plan` command is the wrong contract, or that later reconcile work needs a different typed order model, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Extract the canonical schema model out of `schema_compare` into a dedicated module, for example:
  - `crates/runner/src/validated_schema.rs`
- The extracted model should hold only the facts needed across stories:
  - table names
  - ordered columns
  - PostgreSQL raw column types
  - nullability
  - primary key columns
  - foreign key edges and `ON DELETE` action
  - unique constraint structure
  - non-unique index structure
- Add one new helper-plan module, for example:
  - `crates/runner/src/helper_plan.rs`
- The new typed contract should be mapping-scoped and explicit:
  - `MappingHelperPlan`
  - `HelperShadowTablePlan`
  - `ReconcileOrder`
- `MappingHelperPlan` should be built from a selected validated destination schema and must encode:
  - helper table name
  - source table name
  - ordered helper columns
  - helper-table `CREATE TABLE` SQL that keeps only data columns plus nullability
  - primary-key column metadata retained only for an automatic helper-index decision
  - upsert order: parents before children
  - delete order: children before parents
- Shadow-table generation rules must be enforced centrally in `helper_plan`, not scattered through bootstrap:
  - keep data columns
  - keep destination PostgreSQL type text
  - keep nullability
  - drop defaults
  - drop generated expressions
  - drop PK constraints
  - drop unique constraints
  - drop foreign keys
  - drop secondary indexes
  - do not create the minimal PK index as part of base helper-table DDL
- Dependency ordering must be a real typed topological sort over selected tables using FK edges:
  - upsert order uses parent-before-child
  - delete order is the reverse
  - cycles and self-referential cycles fail loudly with a typed error instead of silently producing a bogus order
- Add one public runner command for artifact generation:
  - `runner render-helper-plan --config <path> --mapping <id> --cockroach-schema <path> --postgres-schema <path> --output-dir <dir>`
- That command should:
  - reuse the semantic schema-validation path from task 01
  - build the selected validated destination schema once
  - render helper-plan artifacts without reparsing into duplicate DTOs
  - write per-mapping artifacts such as:
    - `README.md`
    - `helper_tables.sql`
    - `reconcile_order.txt`
- Apply `improve-code-boundaries` aggressively:
  - remove bootstrap-local `HelperTablePlan`
  - stop using `LIKE ... INCLUDING DEFAULTS INCLUDING GENERATED` as the helper-table shape boundary
  - stop loading PK columns through a dedicated bootstrap-only query once the canonical schema model already has them
  - introduce a narrow PostgreSQL catalog loader that converts live catalog rows directly into the canonical schema model instead of keeping ad hoc row DTOs in bootstrap
- Support for multiple destination databases should stay natural through the existing mapping loop:
  - the helper plan remains per mapping
  - artifact rendering remains per mapping
  - bootstrap builds one plan per mapping without any process-global mutable state

## Public Contract To Establish

- `runner render-helper-plan --config <path> --mapping <id> --cockroach-schema <path> --postgres-schema <path> --output-dir <dir>` exits successfully only when the selected mapping schema is semantically compatible and a helper plan can be generated.
- On success, stdout should give a short summary that is scriptable and human-readable:
  - mapping id
  - output directory
  - number of helper tables
  - number of tables in upsert order
  - number of tables in delete order
- The rendered `helper_tables.sql` must produce helper tables that:
  - keep the real table columns and PostgreSQL types
  - preserve column order
  - preserve nullability
  - omit FKs, unique constraints, PK constraints, defaults, generated expressions, and secondary indexes
- The rendered `reconcile_order.txt` must make both orders explicit:
  - `upsert:` one table per line in parent-before-child order
  - `delete:` one table per line in child-before-parent order
- Bootstrap must consume the same `MappingHelperPlan` contract so later reconcile work can reuse the order model without re-deriving it from strings.

## Files And Structure To Add Or Change

- [x] `crates/runner/src/lib.rs`
  - add the `render-helper-plan` subcommand and keep dispatch thin
- [x] `crates/runner/src/error.rs`
  - add typed artifact and helper-plan failure boundaries, including dependency-cycle failure
- [x] `crates/runner/src/validated_schema.rs`
  - move the canonical schema types here so compare, artifact generation, and bootstrap share one model
- [x] `crates/runner/src/schema_compare/mod.rs`
  - reuse the extracted schema model and expose one validated selected-schema path for downstream helper planning
- [x] `crates/runner/src/schema_compare/cockroach_export.rs`
  - keep parser-specific normalization at the edge only
- [x] `crates/runner/src/schema_compare/postgres_export.rs`
  - keep parser-specific normalization at the edge only
- [x] `crates/runner/src/helper_plan.rs`
  - new typed helper shadow DDL and reconcile-order generator
- [x] `crates/runner/src/postgres_bootstrap.rs`
  - replace `LIKE`-based helper-table cloning and dedicated PK-column loading with the shared helper plan
- [x] `crates/runner/tests/cli_contract.rs`
  - assert that help lists `render-helper-plan`
- [x] `crates/runner/tests/helper_plan_contract.rs`
  - new public-contract tests for helper DDL generation, dependency ordering, and cycle errors via the render command
- [x] `crates/runner/tests/bootstrap_contract.rs`
  - assert that runtime-created helper tables drop defaults, generated expressions, FKs, and secondary indexes while keeping columns and supporting composite PK helper-index decisions
- [x] `crates/runner/tests/long_lane.rs`
  - keep the multi-database runtime coverage green after the bootstrap refactor
- [x] `README.md`
  - document the helper-plan render command and explain that runtime bootstrap uses the same generated rules

## TDD Execution Order

### Slice 1: Tracer Bullet For Helper DDL Rendering

- [x] RED: add one failing contract test that runs `runner render-helper-plan` for one selected table and asserts `helper_tables.sql` contains a helper `CREATE TABLE` with only the expected columns, types, and nullability
- [x] GREEN: add the new command, reuse semantic schema validation from task 01, and render the first minimal helper-table artifact plus a short success summary
- [x] REFACTOR: extract the canonical schema model out of `schema_compare` before the next slices create parallel DTOs

### Slice 2: Strip Serving Structure Aggressively

- [x] RED: add failing contract coverage proving that defaults, generated expressions, PK constraints, unique constraints, FKs, and secondary indexes are absent from rendered helper DDL even when present on the real table
- [x] GREEN: generate helper DDL directly from ordered column facts instead of cloning the real table with `LIKE`
- [x] REFACTOR: keep all shadow-table shape rules in `helper_plan` so bootstrap and artifact rendering cannot diverge

### Slice 3: Composite PK Metadata And Automatic Helper Index Decision

- [x] RED: add failing coverage for a composite-PK table proving that the helper plan retains ordered PK-column metadata but does not bake a PK index into base helper-table DDL
- [x] GREEN: carry ordered PK columns through `HelperShadowTablePlan` and use them only for the automatic helper-index path
- [x] REFACTOR: delete the bootstrap-only PK-column lookup once the shared plan owns this data

### Slice 4: Explicit Dependency Ordering

- [x] RED: add failing coverage for parent-child-grandchild tables asserting upsert order is parent-before-child and delete order is reversed
- [x] GREEN: implement typed FK-edge topological sorting in `helper_plan`
- [x] REFACTOR: store the order as a typed `ReconcileOrder` instead of assembling string lists across call sites

### Slice 5: Loud Failure On Unsupported Dependency Graphs

- [x] RED: add failing coverage for a cycle or self-referential dependency that should not silently receive a bogus reconcile order
- [x] GREEN: return a typed dependency-cycle error with the involved tables in the message
- [x] REFACTOR: keep graph validation inside `helper_plan` so later reconcile code inherits the same loud-failure rule

### Slice 6: Runtime Bootstrap Uses The Shared Plan

- [x] RED: extend bootstrap integration tests so `runner run` fails if it still copies serving structure or drifts from the rendered helper-plan rules
- [x] GREEN: load destination schema into the canonical schema model, build one `MappingHelperPlan`, execute its helper DDL, and create the optional minimal PK helper index only from plan metadata
- [x] REFACTOR: remove the bootstrap-local helper-table planning code and any duplicate catalog DTOs left behind

### Slice 7: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass and remove any leftover duplicate schema shapes, bootstrap-only helper planning, or stringly dependency-order rendering

## Boundary Review Checklist

- [x] No duplicate canonical schema DTOs remain across compare, helper planning, and bootstrap
- [x] No helper-table shape rule survives only inside bootstrap SQL strings
- [x] No `LIKE ... INCLUDING DEFAULTS INCLUDING GENERATED` survives as the helper-table shape boundary
- [x] No dependency order is derived ad hoc from string lists at call sites
- [x] No cycle or unsupported graph case is swallowed
- [x] No duplicate PK-column discovery path remains after the shared plan owns PK metadata
- [x] The helper-plan contract is reusable by later reconcile work without reparsing schema exports or catalog rows into parallel types

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
