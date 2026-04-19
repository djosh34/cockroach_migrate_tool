# Plan: End-To-End Delete Propagation Through Shadow And Real Tables

## References

- Task: `.ralph/tasks/story-10-e2e-baseline/03-task-e2e-delete-propagation-through-shadow-and-real-tables.md`
- Previous story-10 plan: `.ralph/tasks/story-10-e2e-baseline/02-task-e2e-fk-heavy-initial-scan-and-live-catchup_plans/2026-04-19-fk-heavy-e2e-plan.md`
- Existing long-lane scenarios: `crates/runner/tests/default_bootstrap_long_lane.rs`
- Existing long-lane support:
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
- Delete reconcile coverage: `crates/runner/tests/reconcile_contract.rs`
- Design: `designs/crdb-to-postgres-cdc/03_shadow_table_architecture.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- This task is a real ignored long-lane scenario, not a unit-test-only expansion. It must exercise the shipped public flow:
  - `runner run --config <path>`
  - `source-bootstrap render-bootstrap-script --config <path>`
  - `runner verify --config <path> --mapping <id> --source-url <url> [--allow-tls-mode-disable]`
- The simplest delete model must be proven with a real source delete after CDC setup is complete:
  - the helper shadow table loses the row
  - the real destination table still has the row until the next periodic delete reconcile pass
  - the later reconcile pass removes the real row through SQL
- The existing default single-table `customers` scenario is the right tracer bullet here. The task does not need a new FK graph because delete ordering under dependencies is already covered elsewhere; this task is about proving the shadow-absence model end to end on the real long lane.
- The current shared harness hardcodes `reconcile.interval_secs: 1`. That is too hidden and too fast for a reliable end-to-end proof that helper-shadow deletion happens before real-table deletion. The execution turn should move that timing choice into typed harness input instead of leaving it buried in generic support.
- If the first RED slice proves that the shipped delete model cannot be observed cleanly with a slower reconcile interval and real CDC timing, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- The task markdown, acceptance criteria, and story-10 conventions are treated as approval for the public interfaces and required behaviors.
- Highest-priority behaviors to prove:
  - initial scan lands rows in helper shadow and real tables
  - a later source delete removes the row from helper shadow first
  - the real destination row disappears only after the periodic delete reconcile pass
  - repeated reconcile passes do not reinsert or resurrect the deleted row
  - `runner verify` reports a match on the real migrated table after deletion convergence

## Interface And Boundary Decisions

- Keep all product CLIs unchanged.
- Keep the delete scenario on the existing ignored long-lane test crate rather than creating a new integration crate.
- Extend the test-only `CdcE2eHarnessConfig` with one typed timing control:
  - `reconcile_interval_secs`
- Keep generic support responsible for infrastructure and timing:
  - Docker lifecycle
  - temp config generation
  - wrapper binaries
  - runner startup and polling
  - generic helper-shadow and destination queries
- Deepen the single-table customers support boundary so default-bootstrap and delete-propagation scenarios do not duplicate customer snapshot SQL, helper-shadow checks, or source-mutation glue.
- Keep the test file focused on behavior assertions and scenario order. It should not own hidden timing hacks or stringly config assembly.

## Public Contract To Establish

- One ignored long-lane delete scenario proves the selected delete model end to end on a real CockroachDB to PostgreSQL migration:
  - source schema and destination schema both contain the real `public.customers` table
  - initial scan lands the seeded source rows into helper shadow and then into the real destination table
  - after CDC is live, a real source `DELETE` removes the row from the helper shadow table
  - before the next delete reconcile pass runs, the real destination table still contains the row
  - after the periodic delete reconcile pass runs, the real destination table no longer contains the row
  - additional reconcile intervals leave both helper shadow and real tables stable at zero rows for the deleted key
  - `runner verify` succeeds and mentions only the real migrated table

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - `crates/runner/tests/support/e2e_harness.rs` currently owns a hidden reconcile timing policy while `crates/runner/tests/default_bootstrap_long_lane.rs` owns growing scenario-specific delete expectations. That is the wrong split for a timing-sensitive end-to-end delete proof.
- Required cleanup during execution:
  - move reconcile interval selection into typed harness config
  - keep the generic harness generic instead of encoding one hardcoded polling rhythm
  - deepen the customers scenario support so one-table snapshot SQL and delete helpers live with the customers scenario boundary rather than in the long-lane file
- Preferred cleanup shape:
  - either expand `default_bootstrap_harness.rs` into a real customers-scenario support module with multiple callers
  - or replace it with a more honest customers-scenario support module if the current thin wrapper remains a one-caller helper smell
- Do not add another layer of ad hoc helper functions in the long-lane test just to work around the timing issue. If the support boundary is wrong, fix the boundary.

## Files And Structure To Add Or Change

- [x] `crates/runner/tests/support/e2e_harness.rs`
  - add typed `reconcile_interval_secs` support and any generic polling helpers needed to observe helper-shadow deletion before real-table deletion
- [x] `crates/runner/tests/support/default_bootstrap_harness.rs`
  - deepen or reshape the customers scenario support so both baseline and delete scenarios share customer snapshot and delete-mutation helpers
- [x] `crates/runner/tests/default_bootstrap_long_lane.rs`
  - keep the existing baseline scenario green and add the ignored delete-propagation scenario on the cleaner support boundary
- [x] Product code changes were not needed
  - likely hotspots only if a real bug is exposed:
    - `crates/runner/src/reconcile_runtime/delete.rs`
    - `crates/runner/src/webhook_runtime/persistence.rs`
    - `crates/runner/src/molt_verify/mod.rs`
- [x] No CLI surface expansion was needed
  - the new control is test-harness config only, not a user-facing flag

## TDD Execution Order

### Slice 1: Tracer Bullet For Real Delete Convergence

- [x] RED: add one ignored failing long-lane test on the customers scenario that bootstraps the migration, confirms the seeded row appears in the real destination table, performs a real source delete after CDC setup, and waits for the row to disappear from the real destination table
- [x] GREEN: add only the minimum harness support needed to express that scenario through the real public flow
- [x] REFACTOR: keep reconcile timing configurable on the harness boundary instead of hardcoded inside generic support

### Slice 2: Prove Helper-Shadow Deletion Precedes Real-Table Deletion

- [x] RED: extend the delete scenario to use a slower reconcile interval and assert this sequence:
  - helper shadow row count becomes `0`
  - real destination row count is still `1`
  - only then does the later reconcile pass drive the real row count to `0`
- [x] GREEN: fix only the first real gap exposed by that ordering assertion
- [x] REFACTOR: keep helper-shadow count and destination-row checks behind reusable customers-scenario helpers so the scenario stays readable

### Slice 3: Prove Repeated Delete Reconcile Is Stable

- [x] RED: extend the scenario to wait through additional reconcile intervals after convergence and assert the deleted row stays absent from both helper shadow and real tables
- [x] GREEN: no speculative fixes; make the real pipeline pass as built
- [x] REFACTOR: reuse generic polling helpers instead of adding one-off sleeps and duplicated query strings in the test body

### Slice 4: Real MOLT Verify After Delete Convergence

- [x] RED: run `runner verify` for the customers mapping after the delete converges and assert the output reports a matched verdict on the real migrated table only
- [x] GREEN: adjust only the real code or harness support needed for the existing verify command to pass
- [x] REFACTOR: keep verify execution at the CLI/output boundary

### Slice 5: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so the delete scenario does not leave behind a thinner wrapper plus a fatter test file

## TDD Guardrails For Execution

- Every new behavior assertion must fail before code changes are added. If a proposed assertion already passes, replace it with the next uncovered behavior.
- Do not fake webhook payloads, the reconcile loop, or MOLT in the final ignored long-lane scenario.
- Do not add post-bootstrap raw source commands other than the planned source delete that models the application behavior under test.
- Do not hide timing races behind broad sleeps. Express the intended sequencing through typed harness config and observable polling.
- Do not silently ignore Docker, bootstrap, runner, or verify failures. Any such failure is a real task failure.
- If the customers-specific support remains a thin one-caller wrapper after the delete scenario lands, flatten it or rename it so the boundary matches real ownership.

## Boundary Review Checklist

- [x] Reconcile interval is selected at the harness input boundary, not buried in generic support
- [x] The long-lane test reads as operator behavior, not timing and SQL plumbing
- [x] Customers snapshot SQL and delete helpers are not duplicated across scenarios
- [x] Helper-shadow absence is asserted directly instead of inferred only from the final real-table state
- [x] `runner verify` remains at the public CLI boundary
- [x] No real-path error is swallowed

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
