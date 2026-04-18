# Plan: Build The Continuous Upsert Reconcile Loop From Shadow To Real Tables

## References

- Task: `.ralph/tasks/story-07-reconcile/01-task-build-continuous-upsert-reconcile-loop.md`
- Previous task plan: `.ralph/tasks/story-06-destination-ingest/03-task-persist-resolved-watermarks-and-stream-state_plans/2026-04-18-resolved-watermark-and-stream-state-plan.md`
- Next tasks:
  - `.ralph/tasks/story-07-reconcile/02-task-build-continuous-delete-reconcile-pass.md`
  - `.ralph/tasks/story-07-reconcile/03-task-track-reconciled-watermarks-and-repeatable-sync-state.md`
- Design: `designs/crdb-to-postgres-cdc/03_shadow_table_architecture.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/05_design_decisions.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Current implementation:
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/postgres_bootstrap.rs`
  - `crates/runner/src/helper_plan.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/routing.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
  - `crates/runner/src/webhook_runtime/tracking.rs`
- Current tests:
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/runner/tests/long_lane.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The public runtime contract remains `runner run --config <path>`. No new manual reconcile command should be introduced for the production path because the design requires the same destination binary to expose HTTPS and run reconcile continuously.
- Task 01 owns only the continuous upsert reconcile pass. It must not implement the delete anti-join path from story-07 task 02.
- Task 01 may update existing success-oriented tracking rows enough to satisfy this task's acceptance criteria, but it must not invent a second tracking model. It should reuse `_cockroach_migration_tool.stream_state` and `_cockroach_migration_tool.table_sync_state`.
- Task 03 still owns the stricter "how far are we caught up?" semantics: restart-oriented reconcile progress guarantees, reconciled-watermark policy, and richer repeatable-state/error behavior. Task 01 should only do the minimum success-path state advancement that naturally falls out of a successful upsert pass.
- The current implementation already knows the mapping-to-destination connection, helper shadow tables, and dependency order during bootstrap. Reconcile should reuse that typed metadata instead of rediscovering tables or rebuilding names from raw strings.
- If the first execution slices prove that task 01 cannot satisfy acceptance without fully claiming task 03's restart/error/watermark scope, or that a manual reconcile trigger is required to make the public runtime work, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Keep one runtime, not two:
  - `runner run` bootstraps PostgreSQL helper state
  - starts the HTTPS webhook server
  - starts the continuous reconcile worker
  - both use the same canonical mapping runtime plan
- Flatten the current boundary problem identified by `improve-code-boundaries`:
  - today `bootstrap_postgres` returns only `MappingHelperPlan`
  - then `webhook_runtime::routing` rebuilds destination/runtime facts into `MappingWebhookRoute`
  - adding reconcile on top of that would create a third representation of the same mapping
  - task 01 should replace that split ownership with one canonical runtime plan per mapping
- The canonical mapping runtime plan should carry:
  - mapping id
  - source database
  - destination connection
  - helper shadow table metadata
  - dependency-ordered reconcile table metadata for upsert
  - interval configuration needed by the reconcile loop
- The reconcile loop should be its own deep module, separate from webhook handling:
  - webhook code remains responsible for receiving and persisting helper-shadow mutations
  - reconcile code remains responsible for copying helper-shadow truth into real constrained tables
  - shared mapping/runtime metadata lives outside both, so neither HTTP nor reconcile has to rebuild the same facts
- Upsert SQL should be generated from typed table plans, not ad hoc stringly joins spread across the runtime:
  - one typed plan should know the real table name
  - the matching helper shadow table name
  - the ordered column list
  - the primary-key conflict target
- Reconcile execution should be repeatable and timer-driven:
  - run one full upsert pass per mapping on a fixed interval
  - each pass processes mapped tables in dependency order
  - the loop must keep running until the process exits

## Public Contract To Establish

- Starting `runner run` automatically starts continuous upsert reconcile. There is no separate operator-triggered step required after boot.
- After row batches land in helper shadow tables, repeated reconcile passes converge the real target tables toward the helper shadow state without disabling PKs, FKs, or serving indexes.
- Parent tables reconcile before child tables so FK-constrained inserts/updates succeed through the normal real-table constraints.
- Repeated upsert passes are idempotent: rerunning the same pass without helper changes does not corrupt or duplicate real-table state.
- Successful full upsert passes advance existing success-path tracking state in PostgreSQL helper tables without swallowing errors.
- Multiple mappings remain isolated:
  - each mapping reconciles only its own destination database
  - rows from mapping `app-a` never apply into mapping `app-b`

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this task:
  - runtime facts are currently split between bootstrap (`MappingHelperPlan`) and webhook routing (`MappingWebhookRoute`), with reconcile not implemented yet
  - if reconcile is added directly on top of that, the codebase will have bootstrap shapes, webhook shapes, and reconcile shapes describing the same mapping
- Required cleanup:
  - promote bootstrap output into a canonical mapping runtime plan reused by webhook routing and reconcile execution
  - expose dependency-ordered reconcile table plans from the helper-plan boundary instead of keeping order trapped in a private text-rendering structure
  - remove dead ownership of `_reconcile_interval_secs` inside `RunnerWebhookPlan`; the interval must either drive a real loop or move into the new runtime plan/orchestrator
- Secondary cleanup:
  - keep helper-table-name rendering in the existing helper-plan boundary
  - keep reconcile SQL rendering in a dedicated reconcile module, not in `lib.rs` or the HTTP handler
  - keep tracking-state success updates in the tracking module so webhook and reconcile do not drift

## Files And Structure To Add Or Change

- [x] `crates/runner/src/lib.rs`
  - change `Command::Run` from "bootstrap then block forever in webhook serve" to an orchestrated runtime that runs webhook and reconcile together
- [x] `crates/runner/src/postgres_bootstrap.rs`
  - keep bootstrap as the schema/helper discovery boundary and fix catalog metadata needed by reconcile planning
- [x] `crates/runner/src/helper_plan.rs`
  - expose typed dependency-ordered reconcile table plans suitable for SQL-driven upsert execution
- [x] `crates/runner/src/webhook_runtime/mod.rs`
  - consume the shared runtime plan instead of rebuilding route-specific mapping state
- [x] `crates/runner/src/webhook_runtime/routing.rs`
  - reduce this module to request routing only; remove any mapping metadata duplication that now belongs in the shared runtime plan
- [x] `crates/runner/src/webhook_runtime/tracking.rs`
  - add the minimal success-path tracking updates needed after a successful upsert pass, if they naturally fit the current helper tables
- [x] `crates/runner/src/reconcile_runtime/mod.rs`
  - new module that owns the continuous timer loop and per-mapping pass orchestration
- [x] `crates/runner/src/reconcile_runtime/upsert.rs`
  - new module that renders and executes upsert SQL from typed reconcile table plans
- [x] `crates/runner/src/error.rs`
  - add typed reconcile runtime/apply errors; no silent retries or swallowed failures
- [x] `crates/runner/src/runtime_plan.rs`
  - add one canonical mapping runtime plan reused by webhook routing and reconcile execution
- [x] `crates/runner/tests/reconcile_contract.rs`
  - new end-to-end contract coverage for continuous upsert reconcile behavior
- [x] `crates/runner/tests/long_lane.rs`
  - no extension was needed because the new contract file covered the multi-mapping and interval behavior directly, while `make test-long` still passed unchanged

## TDD Execution Order

### Slice 1: Tracer Bullet For Continuous Upsert

- [x] RED: add one failing contract test that starts `runner run`, sends one HTTPS row-batch request, waits at least one reconcile interval, and expects the row to appear in the real destination table
- [x] GREEN: implement the smallest runtime orchestration plus one-table upsert reconcile path needed to make that end-to-end test pass
- [x] REFACTOR: extract the reconcile loop entrypoint into its own module so `lib.rs` does not mix CLI orchestration, HTTP startup, and SQL reconcile logic

### Slice 2: Dependency-Ordered Multi-Table Upsert

- [x] RED: add failing coverage for a parent/child schema where helper shadow tables contain rows for both tables and the child row becomes visible in the real table only because reconcile processed the parent first
- [x] GREEN: drive reconcile table iteration from typed dependency order produced by the helper-plan layer
- [x] REFACTOR: expose a canonical ordered reconcile-table plan instead of passing raw table names and looking up helper metadata again during execution

### Slice 3: Repeatable Idempotent Execution

- [x] RED: add failing coverage that allows multiple reconcile intervals to run without helper changes and proves the real table contents remain stable
- [x] GREEN: make repeated upsert passes safe by using `INSERT .. ON CONFLICT DO UPDATE` against the real target tables
- [x] REFACTOR: centralize per-table upsert SQL generation so repeatability logic is not duplicated across test cases or call sites

### Slice 4: Multi-Mapping Isolation

- [x] RED: add failing coverage for two mappings pointing at two destination databases and prove each reconcile worker touches only its own database and tables
- [x] GREEN: run one reconcile loop per mapping from the shared canonical runtime plan
- [x] REFACTOR: remove any remaining duplicated mapping assembly between bootstrap, webhook routes, and reconcile workers

### Slice 5: Success-Path Tracking Updates

- [x] RED: add failing coverage that a successful full upsert pass advances the existing helper-table sync/stream state required by this task's acceptance criteria
- [x] GREEN: update only the minimal existing success-path fields that naturally belong to task 01, keeping richer reconciled-watermark/error semantics for task 03
- [x] REFACTOR: keep reconcile success-state persistence inside the tracking boundary so the reconcile worker only reports typed pass results

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm the new reconcile runtime did not leave duplicate mapping/runtime shapes behind

## Boundary Review Checklist

- [x] No separate manual reconcile command is required for the steady-state runtime
- [x] No dead reconcile interval field remains unused after this task
- [x] No mapping runtime facts are duplicated across bootstrap, webhook routing, and reconcile execution
- [x] No helper-table name or ordered table metadata is rebuilt from raw strings outside the canonical plan boundary
- [x] No reconcile error is swallowed, downgraded, or converted into fake success
- [x] No delete reconcile behavior is implemented early in task 01
- [x] No task-03-only restart/error/watermark semantics are claimed unless execution proves they are unavoidably part of task 01

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
