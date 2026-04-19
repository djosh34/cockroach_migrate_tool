# Plan: End-To-End Composite Primary Keys And Excluded Table Handling

## References

- Task: `.ralph/tasks/story-10-e2e-baseline/04-task-e2e-composite-pk-and-excluded-table-handling.md`
- Previous story-10 plan: `.ralph/tasks/story-10-e2e-baseline/03-task-e2e-delete-propagation-through-shadow-and-real-tables_plans/2026-04-19-delete-propagation-e2e-plan.md`
- Existing long-lane scenarios:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Existing long-lane support:
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
- Existing contract evidence:
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/runner/tests/helper_plan_contract.rs`
  - `crates/runner/tests/verify_contract.rs`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The product already models exclusion through the selected table list, not through a second exclude-list contract. This task should prove that real unselected tables are ignored intentionally rather than inventing a new config shape.
- The composite-PK proof must stay on the real public path:
  - `runner run --config <path>`
  - `source-bootstrap render-bootstrap-script --config <path>`
  - `runner verify --config <path> --mapping <id> --source-url <url> [--allow-tls-mode-disable]`
- The scenario should cover at least:
  - one included simple table for baseline realism
  - one included composite-PK table that sees real end-to-end data movement
  - one excluded table that exists on both sides but is intentionally not selected for migration
- The scenario must prove continuous reconcile, not only initial scan. After CDC setup completes, the test should perform real source mutations on the included tables and separate source mutations on the excluded table.
- The excluded table proof should stay operator-visible:
  - no helper shadow table for the excluded real table
  - no replicated data into the excluded destination table
  - no mention of the excluded table in verify output
- If the first RED slice proves that exclusion cannot be expressed cleanly by the existing selected-table contract, or that composite-PK convergence requires a different helper-shadow or verify boundary than the one already shipped, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- The task markdown and story-10 conventions are treated as approval for the public interface and required behaviors.
- Highest-priority behaviors to prove:
  - composite-PK included tables bootstrap into helper shadow and real destination tables
  - later composite-PK updates, inserts, and deletes converge through the real CDC plus reconcile path
  - excluded tables remain intentionally untouched even while their source rows change
  - helper-shadow artifacts exist only for the included selected tables
  - `runner verify` passes for the intended included real tables only

## Interface And Boundary Decisions

- Keep all product CLIs unchanged.
- Keep exclusion expressed through `source.tables`, not by inventing an `excluded_tables` field.
- Add one new ignored long-lane scenario for this task instead of overloading an existing scenario with many inline SQL constants and assertions.
- Treat scenario support as the primary boundary-cleanup target:
  - the current `crates/runner/tests/default_bootstrap_long_lane.rs` already carries multiple unrelated schemas and large embedded SQL strings
  - this task should move composite/exclusion schema setup, mutation helpers, and snapshot assertions into a dedicated support module rather than making the long-lane file fatter
- Keep the generic harness generic:
  - container lifecycle
  - temp config generation
  - wrapper binaries
  - generic polling and query helpers
  - verify command execution
- Keep scenario support responsible for:
  - included/excluded schema fixtures
  - scenario-specific snapshot SQL
  - source mutation helpers
  - readable assertions about included and excluded state

## Public Contract To Establish

- One ignored long-lane scenario proves that a realistic mixed schema behaves correctly end to end:
  - the source contains at least one included simple table, one included composite-PK table, and one excluded table
  - the destination contains corresponding real tables
  - initial scan lands only the included tables into helper shadow and real destination tables
  - the excluded destination table stays empty even if the source excluded table contains rows
  - after CDC is live, source mutations on the included tables converge into helper shadow and then real destination tables
  - after CDC is live, source mutations on the excluded table remain absent from helper shadow and absent from the excluded destination table
  - repeated reconcile passes leave the included tables stable and do not suddenly materialize excluded-table rows
  - `runner verify` succeeds and mentions only the included real tables

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - `crates/runner/tests/default_bootstrap_long_lane.rs` is becoming a story-wide dumping ground for unrelated schema fixtures, mutation SQL, and assertion plumbing
- Required cleanup during execution:
  - extract the new scenario into a dedicated support boundary instead of adding another large block of schema SQL and stringly snapshots to the long-lane file
  - keep excluded-table checks owned by scenario support, not by ad hoc helper-table name string building in the test body
  - if the existing `default_bootstrap_harness.rs` becomes misleading because it only covers the customers scenario, either keep it customers-specific and add a second honest support module, or rename/reshape the support boundary so ownership is obvious
- Do not solve this by adding another layer of one-off helper functions directly in the test file. If the boundary is wrong, fix the boundary.

## Files And Structure To Add Or Change

- [x] `crates/runner/tests/default_bootstrap_long_lane.rs`
  - add the ignored composite-PK plus excluded-table scenario, or split the scenario into a more honest long-lane file if that reads better after the support extraction
- [x] `crates/runner/tests/support/e2e_harness.rs`
  - only add generic helpers if the scenario truly needs reusable helper-table existence or exact-table listing assertions
- [x] `crates/runner/tests/support/default_bootstrap_harness.rs`
  - keep only if it remains the honest owner of the customers-only baseline and delete scenarios
- [x] `crates/runner/tests/support/composite_pk_exclusion_harness.rs`
  - preferred new support boundary for this task's schema, snapshots, and source-mutation helpers
- [x] Product code changes likely are not needed
  - probable hotspots only if the long-lane RED slice exposes a real bug:
    - `crates/runner/src/helper_plan.rs`
    - `crates/runner/src/reconcile_runtime/upsert.rs`
    - `crates/runner/src/reconcile_runtime/delete.rs`
    - `crates/runner/src/molt_verify/mod.rs`
- [x] No new public config fields should be added unless the RED slice proves the current selected-table contract is insufficient

## TDD Execution Order

### Slice 1: Tracer Bullet For Composite-PK Initial Scan With An Excluded Table Present

- [x] RED: add one ignored failing long-lane scenario that bootstraps a source containing an included composite-PK table and an excluded table, then assert the included composite table appears in helper shadow and the real destination while the excluded table does not
- [x] GREEN: add only the minimum scenario-support code needed to express that end-to-end path through the real public commands
- [x] REFACTOR: move schema fixtures and snapshot SQL behind a dedicated scenario support boundary instead of leaving them inline in the long-lane file

### Slice 2: Prove Continuous Reconcile On The Included Composite-PK Table

- [x] RED: extend the scenario with real source mutations after CDC setup on the included tables:
  - update an existing composite-PK row
  - insert a new composite-PK row
  - delete a composite-PK row
- [x] GREEN: make only the real code or harness changes needed for the end-to-end convergence to pass
- [x] REFACTOR: keep the composite-table snapshot and mutation helpers in scenario support so the test body reads as behavior, not SQL plumbing

### Slice 3: Prove Excluded Tables Stay Intentionally Ignored

- [x] RED: extend the same scenario with source writes to the excluded table and assert all of the following:
  - no helper shadow table exists for the excluded table, or it never appears in the helper-table inventory
  - the excluded destination table remains unchanged
  - the included tables still converge correctly
- [x] GREEN: fix only the first real gap the scenario exposes
- [x] REFACTOR: keep excluded-table existence and snapshot assertions on a reusable support boundary rather than scattered stringly table-name checks

### Slice 4: Real Verify Coverage For The Included Set Only

- [x] RED: run `runner verify` after convergence and assert the output reports a matched verdict while mentioning only the included selected tables
- [x] GREEN: adjust only the real code or harness support needed for the existing verify command to pass
- [x] REFACTOR: keep verify assertions at the CLI/output boundary and do not introduce test-only verify shortcuts

### Slice 5: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so the new scenario support is honest and the long-lane file is not regressing into a fixture dump

## Candidate Scenario Shape

- Included simple table: `public.customers`
- Included composite-PK table: `public.order_items`
- Excluded table: `public.audit_events`
- Example included assertions:
  - customers snapshot converges
  - order-items snapshot converges with composite keys preserved
  - helper shadow row counts exist for `public.customers` and `public.order_items`
- Example excluded assertions:
  - helper-table inventory contains only the included helper tables
  - `public.audit_events` in the destination stays `<empty>` despite source-side inserts or updates
- This exact schema can change during RED if a simpler mixed-schema tracer bullet proves the same contract with less incidental complexity.

## TDD Guardrails For Execution

- Every new behavior assertion must fail before code changes are added. If a proposed assertion already passes, replace it with the next uncovered behavior.
- Do not fake webhook payloads, helper-shadow contents, or MOLT output in the final ignored long-lane scenario.
- Do not add extra raw source commands after CDC setup except the planned included-table and excluded-table mutations that model the behavior under test.
- Do not invent a second exclusion contract unless the RED slice proves the selected-table contract is actually wrong.
- Do not silently ignore Docker, bootstrap, runner, or verify failures. Any such failure is a real task failure.
- If the new support module ends up being a thin wrapper over generic harness calls, flatten it or rename it so the boundary matches real ownership.

## Boundary Review Checklist

- [x] The long-lane test reads as operator behavior, not schema-fixture and SQL assembly plumbing
- [x] Composite-PK snapshot and mutation SQL are not duplicated across the test file and support code
- [x] Excluded-table checks are expressed directly, not inferred only from final verify output
- [x] Helper-table inventory assertions stay on a reusable support boundary
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
