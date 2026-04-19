# Plan: Emit The Absolute Minimum PostgreSQL Runner Grants

## References

- Task: `.ralph/tasks/story-19-source-sql-emitter-image/03-task-emit-the-absolute-minimum-postgresql-role-grants-needed-by-the-runner.md`
- Related prior plans:
  - `.ralph/tasks/story-19-source-sql-emitter-image/01-task-build-a-one-time-sql-emitter-image-that-prints-required-sql-to-logs_plans/2026-04-19-one-time-setup-image-plan.md`
  - `.ralph/tasks/story-19-source-sql-emitter-image/02-task-emit-the-required-cockroach-changefeed-sql-from-the-one-time-setup-image_plans/2026-04-19-cockroach-changefeed-sql-plan.md`
- Current PostgreSQL grant surface:
  - `crates/setup-sql/src/lib.rs`
  - `crates/setup-sql/src/config/postgres_grants.rs`
  - `crates/setup-sql/src/config/postgres_grants_parser.rs`
  - `crates/setup-sql/src/render/postgres_grants.rs`
  - `crates/setup-sql/tests/bootstrap_contract.rs`
- Current runner privilege consumers:
  - `crates/runner/src/postgres_bootstrap.rs`
  - `crates/runner/src/tracking_state.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
  - `crates/runner/src/reconcile_runtime/upsert.rs`
  - `crates/runner/src/reconcile_runtime/delete.rs`
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/readme_contract.rs`
- Public docs:
  - `README.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public `emit-postgres-grants` surface and the required behavior priorities.
- The PostgreSQL grant command remains a setup-image concern only:
  - it emits SQL
  - it does not apply SQL
  - it does not require Cockroach or webhook config
- The JSON contract stays simple:
  - one top-level object
  - one SQL string per destination database
  - no extra wrappers, grant lists, or metadata arrays
- The operator-configured runtime role name remains supported through `mappings[].destination.runtime_role`.
- The current renderer is already close on command shape, but it is not yet least-privileged and it still mixes grouping, grant planning, and string rendering in one file.
- If a RED slice proves the runner needs a privilege not currently justified by the source inspection below, switch this plan back to `TO BE VERIFIED` and stop execution immediately.

## Minimum Privilege Target

- The runner must be able to connect to each configured destination database.
- The runner must be able to create the helper schema and helper tables under `_cockroach_migration_tool`.
  - `crates/runner/src/postgres_bootstrap.rs` creates the schema and tables itself.
  - This implies `CREATE` on the database is required.
- The runner must be able to use the mapped application schema and mutate mapped tables.
  - bootstrap reads catalog metadata for mapped tables
  - webhook persistence writes helper tables
  - reconcile writes mapped tables via `INSERT ... ON CONFLICT DO UPDATE`
  - reconcile deletes from mapped tables and references target columns in the delete predicate
- The smallest supported emitted grants should therefore be:
  - `GRANT CONNECT, CREATE ON DATABASE ... TO ...;`
  - `GRANT USAGE ON SCHEMA public TO ...;`
  - `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE ... TO ...;` for each mapped table
- The emitted SQL must not include:
  - `TEMPORARY`
  - `SUPERUSER`
  - `CREATE ROLE`
  - broad `ALL PRIVILEGES`
  - schema-wide or database-wide table grants such as `ALL TABLES IN SCHEMA public`

## Problem To Fix

- `crates/setup-sql/src/render/postgres_grants.rs` currently emits broader privileges than the task allows:
  - `TEMPORARY` on the database
  - direct string assembly without a typed grant-plan boundary
- The existing tests in `crates/setup-sql/tests/bootstrap_contract.rs` encode the broader contract, so they need to be driven RED first and then narrowed.
- The runner integration tests still seed destination permissions with broader grants in a few places, which leaves the minimum privilege claim unproved.
- The current output comments are serviceable, but the render module is doing too much at once:
  - group mappings by database
  - decide the required grant statements
  - render text output
  - render JSON output

## Interface And Boundary Decisions

- Keep the public CLI:
  - `setup-sql emit-postgres-grants --config <path> [--format text|json]`
- Keep the PostgreSQL-only config boundary:
  - database
  - runtime role
  - mapped tables
  - no Cockroach fields
- Keep JSON simple and stable:
  - destination database name is the key
  - value is a single SQL string for that database
- Keep plain-text output as SQL plus SQL comments only.
- Introduce a typed per-database grant plan behind the renderer.
  - The plan should own the exact grant statements.
  - Text and JSON should both render from that same plan.
- Reduce wrong-place knowledge:
  - validation remains in `config/postgres_grants_parser.rs`
  - privilege decisions move into a typed PostgreSQL grant-plan layer
  - final string formatting stays at the render boundary

## Improve-Code-Boundaries Focus

- Primary smell: `crates/setup-sql/src/render/postgres_grants.rs` mixes planning and rendering.
  - Extract a typed per-database grant plan or statement enum so the minimum privilege policy is represented once.
  - Use `Display` or equivalent focused rendering for grant statements instead of free-form string soup spread across branches.
- Primary smell: mixed command-domain exports in `crates/setup-sql/src/config/mod.rs`.
  - Reduce the cross-command grab bag so the PostgreSQL grant path depends only on its own validated config types.
  - Avoid adding more public reexports that blur Cockroach and PostgreSQL ownership.
- Secondary smell: test evidence for minimum privileges currently lives mostly in output-string assertions.
  - Add runner contract coverage that proves the emitted privilege floor is sufficient for real startup and reconcile behavior.

## Public Contract To Establish

- `setup-sql --help` still lists `emit-postgres-grants`.
- `emit-postgres-grants`:
  - requires only PostgreSQL grant config
  - supports `text` and `json`
  - supports multiple destination databases in one invocation
  - preserves configurable runtime role names
- Text output:
  - contains only the minimum supported privileges
  - may include SQL comments
  - must not contain shell artifacts or role-creation SQL
- JSON output:
  - remains a top-level object keyed by destination database
  - each value is one SQL string for that database
  - preserves the same minimum grant contract as text mode
- Runner behavior:
  - still bootstraps helper schema/tables
  - still reconciles data successfully when only the emitted minimum grants are present
  - does not require `TEMPORARY`, `ALL PRIVILEGES`, or schema-wide blanket grants

## Files And Structure To Add Or Change

- [x] `.ralph/tasks/story-19-source-sql-emitter-image/03-task-emit-the-absolute-minimum-postgresql-role-grants-needed-by-the-runner.md`
  - add the execution-plan pointer for this task
- [x] `crates/setup-sql/src/render/postgres_grants.rs`
  - narrow the emitted privileges and split typed grant planning from final rendering
- [x] `crates/setup-sql/src/render/mod.rs`
  - keep the PostgreSQL render boundary thin after the refactor
- [x] `crates/setup-sql/src/lib.rs`
  - keep command dispatch thin after any PostgreSQL render/planning cleanup
- [x] `crates/setup-sql/src/config/mod.rs`
  - reduce cross-command export mud if the PostgreSQL path can depend on a smaller validated surface
- [x] `crates/setup-sql/tests/bootstrap_contract.rs`
  - drive RED on least-privilege output and multi-database JSON preservation
- [x] `crates/setup-sql/tests/fixtures/valid-postgres-grants-config.yml`
  - keep or extend only if extra coverage needs an additional mapped table/database shape
- [x] `crates/runner/tests/bootstrap_contract.rs`
  - replace over-broad seed grants with the minimum grants where that proves the runtime contract honestly
- [x] `crates/runner/tests/readme_contract.rs`
  - keep README assertions aligned if the documented SQL examples change
- [x] `README.md`
  - document the narrowed PostgreSQL grant output if the quick start currently implies broader privileges

## TDD Execution Order

### Slice 1: Tracer Bullet For Least-Privilege Text Output

- [x] RED: tighten one existing `emit-postgres-grants` text-mode contract test so it fails unless the database grant drops `TEMPORARY`, keeps `CREATE`, and forbids blanket privilege forms such as `ALL PRIVILEGES`
- [x] GREEN: implement the minimum renderer change to emit the narrowed database grant SQL
- [x] REFACTOR: introduce a small typed grant-statement boundary so the least-privilege decision is not buried in `format!` calls

### Slice 2: Preserve The Same Minimum Contract In JSON

- [x] RED: tighten the JSON contract test so the per-database SQL string also excludes `TEMPORARY` and any broad grant forms while preserving one SQL string per database
- [x] GREEN: render JSON from the same typed per-database plan used by text mode
- [x] REFACTOR: delete any duplicated privilege-string assembly between text and JSON rendering

### Slice 3: Prove The Grants Are Sufficient For Real Runner Bootstrap

- [x] RED: add one runner bootstrap contract that provisions a destination role with only the emitted minimum grants and fails if startup cannot create helper schema/tables and seed tracking state
- [x] GREEN: adjust the emitted SQL or the test setup until the traced minimum grant set is both sufficient and still narrower than today
- [x] REFACTOR: replace any remaining over-broad bootstrap test setup grants that no longer represent the supported contract

### Slice 4: Prove Reconcile Does Not Need Extra Privileges

- [x] RED: add or tighten one reconcile- or webhook-adjacent contract proving the runtime can still mutate mapped tables and track helper state with the narrowed grant set
- [x] GREEN: keep only the privileges justified by the real SQL paths
- [x] REFACTOR: remove stale assumptions in tests or docs that still imply `ALL PRIVILEGES`, `TEMPORARY`, or schema-wide blanket grants

### Slice 5: Boundary Cleanup

- [x] RED: let `make check` or clippy/format failures force any API cleanup from the render-plan extraction
- [x] GREEN: reduce `config/mod.rs` and the PostgreSQL render surface so command-specific knowledge stays in the right module
- [x] REFACTOR: prefer `Display`-style rendering for typed grant statements and delete dead helper code or duplicate string-building branches

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, and `make test`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required default lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so the PostgreSQL grant path is not left muddier than it started

## TDD Guardrails For Execution

- Start with one failing public-behavior test before each product change. Do not batch speculative tests.
- Keep tests at public boundaries:
  - CLI output
  - config-loading behavior
  - runner startup/reconcile contract
  - README contract where applicable
- Do not add compatibility grants just to satisfy old tests.
- Do not silently tolerate privilege uncertainty. If the runtime needs a new privilege, prove it with a failing test and update the minimum contract explicitly.
- Do not swallow any errors or downgrade permission failures into vague strings.

## Boundary Review Checklist

- [x] PostgreSQL privilege decisions live in one typed plan boundary instead of ad hoc string assembly
- [x] Text and JSON output share one source of truth for the minimum grant set
- [x] Cockroach and PostgreSQL command ownership stays separated
- [x] No broad grant forms survive in emitted SQL without a concrete runtime proof
- [x] Runner tests that claim least privilege no longer seed broader grants than the emitted contract
- [x] No error path is swallowed or hidden behind generic output

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] One final `improve-code-boundaries` pass after all required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-19-source-sql-emitter-image/03-task-emit-the-absolute-minimum-postgresql-role-grants-needed-by-the-runner_plans/2026-04-19-postgres-minimum-grants-plan.md`

NOW EXECUTE
