# Plan: Build The Continuous Delete Reconcile Pass From Shadow To Real Tables

## References

- Task: `.ralph/tasks/story-07-reconcile/02-task-build-continuous-delete-reconcile-pass.md`
- Previous task plan: `.ralph/tasks/story-07-reconcile/01-task-build-continuous-upsert-reconcile-loop_plans/2026-04-18-continuous-upsert-reconcile-loop-plan.md`
- Neighbor task:
  - `.ralph/tasks/story-07-reconcile/03-task-track-reconciled-watermarks-and-repeatable-sync-state.md`
- Design: `designs/crdb-to-postgres-cdc/03_shadow_table_architecture.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/05_design_decisions.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Design: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Current implementation:
  - `crates/runner/src/helper_plan.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/reconcile_runtime/upsert.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
  - `crates/runner/src/webhook_runtime/tracking.rs`
  - `crates/runner/src/error.rs`
- Current tests:
  - `crates/runner/tests/reconcile_contract.rs`
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/runner/tests/helper_plan_contract.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The public runtime contract remains `runner run --config <path>`. Delete reconcile must be part of the already-running continuous worker, not a separate CLI mode or operator-triggered command.
- The webhook runtime already removes rows from helper shadow tables on delete row-batches. This task only propagates helper-table absence into the real constrained tables.
- The runtime should treat one interval as one full reconcile pass:
  - upsert real tables from helper truth in parent-before-child order
  - delete real rows missing from helper truth in child-before-parent order
  - advance success-path tracking only after the full pass succeeds
- This task must not add tombstones, soft deletes, or side-channel delete journals. The selected design remains "helper shadow tables are the truth; SQL anti-join deletes propagate absence."
- Task 03 still owns any richer restart/error policy, reconciled-watermark semantics, or partial-pass state machine. Task 02 should only preserve the existing "persist success after a full successful pass" model.
- If the first execution slice proves that delete propagation cannot be expressed from the existing helper metadata without inventing a second planning model, or that tracking must become a more complex state machine to stay correct, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Keep one canonical reconcile plan per mapping:
  - runtime bootstrap builds the plan once from `MappingHelperPlan`
  - webhook routing/persistence keep using helper-table metadata
  - reconcile execution consumes typed upsert and delete orders from the same runtime plan
- Flatten the current boundary problem identified by `improve-code-boundaries`:
  - today `MappingRuntimePlan` exposes only `reconcile_tables()`, which is really "upsert order"
  - delete support would otherwise force `reconcile_runtime` to reverse vectors ad hoc or rediscover table order from `helper_plan`
  - task 02 should replace that vague boundary with explicit typed reconcile ordering in the runtime plan
- Reconcile orchestration should stay shallow:
  - `reconcile_runtime/mod.rs` owns worker startup and one-pass sequencing
  - `reconcile_runtime/upsert.rs` owns only upsert SQL application
  - `reconcile_runtime/delete.rs` should own only delete SQL application
  - tracking remains in `webhook_runtime/tracking.rs`
- Delete SQL should be rendered from typed table metadata, not string soup rebuilt in the worker:
  - target real table name
  - helper shadow table name
  - ordered primary-key columns
  - SQL shape using `DELETE ... WHERE NOT EXISTS (...)` or equivalent anti-join
- Delete order must come from the canonical helper-plan dependency graph, not from local worker heuristics:
  - upsert order remains parent-before-child
  - delete order remains child-before-parent
- The full reconcile pass should stay transactional per mapping. If any upsert or delete statement fails, the whole pass rolls back and success tracking must not advance.

## Public Contract To Establish

- Starting `runner run` continuously converges real tables toward helper shadow truth for both inserts/updates and deletes.
- When a row disappears from a mapping's helper shadow table, a later reconcile interval deletes the corresponding row from the real table.
- Parent/child schemas delete in reverse dependency order so FK-constrained tables do not fail on parent removal.
- Repeating the same delete reconcile pass without new helper changes is safe and idempotent.
- Successful full passes keep advancing the existing helper tracking rows without swallowing any SQL/runtime failures.
- Multiple mappings remain isolated:
  - each mapping only touches its own helper tables and destination database
  - deletes for mapping `app-a` must never remove rows from mapping `app-b`

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this task:
  - `helper_plan` already computes both upsert and delete order
  - `runtime_plan` currently downgrades that into one ambiguous upsert-only `reconcile_tables` vector
  - delete execution would become a "wrong place" runtime concern unless the canonical plan carries both orders directly
- Required cleanup:
  - expose explicit `reconcile_upsert_tables` and `reconcile_delete_tables` behavior from the canonical runtime boundary
  - keep delete SQL rendering in `reconcile_runtime/delete.rs`, not in `mod.rs` or `runtime_plan.rs`
  - avoid duplicating "all tables in this mapping" shapes just to feed tracking; reuse canonical typed table plans where possible
- Secondary cleanup:
  - keep `reconcile_runtime/mod.rs` focused on interval workers and pass sequencing, not SQL generation
  - keep typed error variants phase-specific so failures stay loud and actionable
  - do not invent a helper function pile with one caller per phase; inline trivial orchestration where it clarifies the pass

## Files And Structure To Add Or Change

- [x] `crates/runner/src/runtime_plan.rs`
  - replace the ambiguous upsert-only `reconcile_tables` boundary with explicit typed upsert/delete reconcile order accessors
- [x] `crates/runner/src/helper_plan.rs`
  - expose delete-order metadata for runtime planning instead of leaving it trapped in artifact rendering text
- [x] `crates/runner/src/reconcile_runtime/mod.rs`
  - orchestrate one full pass per interval as `upsert -> delete -> persist success`
- [x] `crates/runner/src/reconcile_runtime/delete.rs`
  - new module that renders and executes anti-join delete SQL from typed table plans
- [x] `crates/runner/src/reconcile_runtime/upsert.rs`
  - keep this module focused on upserts only; adjust any shared helpers if full-pass orchestration needs a cleaner boundary
- [x] `crates/runner/src/webhook_runtime/tracking.rs`
  - ensure success-path tracking still reflects completion of the full combined pass and not just half of it
- [x] `crates/runner/src/error.rs`
  - add typed delete-apply failure boundaries if the current reconcile error enum no longer describes failures precisely
- [x] `crates/runner/tests/reconcile_contract.rs`
  - add end-to-end contract coverage for continuous delete propagation, reverse dependency order, idempotence, and mapping isolation
- [x] `crates/runner/tests/helper_plan_contract.rs`
  - extend only if needed to keep the canonical delete-order boundary explicitly covered at the artifact/planning level

## TDD Execution Order

### Slice 1: Tracer Bullet For Real-Table Delete Propagation

- [x] RED: add one failing contract test that starts `runner run`, ingests a row into helper shadow state, waits for real-table upsert, then sends a delete row-batch and expects the real row to disappear after a later reconcile interval
- [x] GREEN: implement the smallest full-pass delete reconcile path needed to make that end-to-end behavior pass
- [x] REFACTOR: extract delete SQL application into `reconcile_runtime/delete.rs` so `mod.rs` stays orchestration-only

### Slice 2: Child-Before-Parent Delete Order

- [x] RED: add failing coverage for a parent/child schema where both helper rows disappear and the real tables only reconcile successfully if the child row is deleted before the parent row
- [x] GREEN: drive delete iteration from typed delete order produced by the helper-plan/runtime-plan boundary
- [x] REFACTOR: remove any local reversing or order reconstruction from the worker once the canonical plan exposes delete order directly

### Slice 3: Repeatable Idempotent Delete Passes

- [x] RED: add failing coverage that allows multiple reconcile intervals to run after helper-row deletion and proves the real tables remain empty/stable rather than erroring or mutating unexpectedly
- [x] GREEN: ensure the delete SQL is naturally idempotent by deleting only rows absent from helper truth
- [x] REFACTOR: centralize anti-join delete SQL rendering so idempotence logic is not duplicated across call sites

### Slice 4: Multi-Mapping Delete Isolation

- [x] RED: add failing coverage for two mappings with distinct databases and prove delete reconcile for one mapping does not remove rows from the other mapping
- [x] GREEN: keep one worker per mapping and run delete passes only against that mapping's destination connection and table plans
- [x] REFACTOR: remove any remaining mapping-specific branching from shared reconcile helpers if the test exposes it

### Slice 5: Success Tracking After Full Pass

- [x] RED: add failing coverage that a successful interval containing delete reconciliation still advances the existing stream/table sync state only after the full pass succeeds
- [x] GREEN: keep tracking updates after both phases complete successfully inside the transaction
- [x] REFACTOR: make the "tables whose sync state advances" boundary explicit so tracking does not care whether the pass performed upserts, deletes, or both

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm delete reconcile did not leave behind duplicate ordering or SQL-planning shapes

## Boundary Review Checklist

- [x] No separate delete command or operator step is introduced
- [x] No tombstone or side-channel delete state is added early
- [x] No reconcile order is recomputed ad hoc outside the canonical planning boundary
- [x] No SQL delete rendering is mixed into runtime startup or tracking code
- [x] No reconcile error is swallowed, downgraded, or converted into fake success
- [x] No success-path tracking update happens before the full upsert-plus-delete transaction succeeds
- [x] No mapping runtime facts are duplicated just to support delete order

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
