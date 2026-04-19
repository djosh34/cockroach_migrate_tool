# Plan: End-To-End High Source Write Churn During Transfer

## References

- Task: `.ralph/tasks/story-11-e2e-chaos/05-task-e2e-source-high-write-churn-during-transfer.md`
- Previous story-11 execution plans:
  - `.ralph/tasks/story-11-e2e-chaos/03-task-e2e-network-fault-injection-imposed-externally_plans/2026-04-19-external-network-fault-e2e-plan.md`
  - `.ralph/tasks/story-11-e2e-chaos/04-task-e2e-transaction-failure-recovery_plans/2026-04-19-transaction-failure-recovery-plan.md`
- Existing long-lane scenarios:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Existing long-lane support:
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/runner_process.rs`
- Runtime surfaces most likely to expose correctness bugs under churn:
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/reconcile_runtime/upsert.rs`
  - `crates/runner/src/tracking_state.rs`
- Design and test strategy:
  - `designs/crdb-to-postgres-cdc/06_recommended_design.md`
  - `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown plus `06_recommended_design.md` and `07_test_strategy.md` are treated as approval for the interface and behavior priorities in this turn.
- This task must stay on the real public path:
  - `runner run --config <path>`
  - `source-bootstrap render-bootstrap-script --config <path>`
  - `runner verify --config <path> --mapping <id> --source-url <url> [--allow-tls-mode-disable]`
- No hidden runner hook, fake webhook payload, alternate apply path, or manual source-side repair step may be added.
- The churn workload must be imposed from outside the runner through real Cockroach SQL after CDC setup is complete.
- The workload must include repeated create/update/delete activity with a deterministic final source state, so the test can prove both eventual convergence and the absence of phantom rows.
- The current harness already exposes enough generic polling, destination querying, helper-shadow querying, tracking-state polling, and verify execution that product-code changes are not expected unless the test exposes a real bug.
- If the planned workload cannot be expressed deterministically through the existing public source SQL helper plus small customer-specific harness extensions, this plan must be switched back to `TO BE VERIFIED` before execution continues.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - the system survives a bounded burst of real source create/update/delete churn during active shadowing
  - helper shadow tables converge to the same final state as the source after the churn settles
  - the real destination tables converge to the same final state without runner restart or manual cleanup
  - tracking state catches up without recording a durable error
  - `runner verify` reports a matched verdict for the real migrated table only
- Lower-priority implementation concerns:
  - keep the long-lane test readable
  - keep the churn script and expected-state modeling inside customer-specific support instead of scattering raw SQL and snapshot strings through the test body

## Interface And Boundary Decisions

- Keep all product CLIs unchanged.
- Keep `CdcE2eHarness` responsible for:
  - Docker lifecycle
  - config materialization
  - source and destination SQL helpers
  - runner startup
  - generic destination polling
  - generic tracking-state polling
  - verify execution
- Keep `DefaultBootstrapHarness` responsible for:
  - customer-specific source mutations
  - customer snapshot expectations
  - customer-specific churn orchestration
  - customer-specific catch-up assertions
- Flatten one likely boundary problem before execution grows the test body:
  - a high-churn scenario will get muddy fast if `default_bootstrap_long_lane.rs` owns raw multi-statement SQL, final expected snapshot assembly, and tracking-watermark assertions inline
  - execution should introduce one typed customer-churn helper boundary in `default_bootstrap_harness.rs` so the long-lane test reads as behavior, not setup plumbing
- Prefer one customer-specific workload/result boundary over a fake-generic mutation DSL in `e2e_harness.rs`.
  - Example direction:
    - `run_high_customer_write_churn_workload() -> CustomerChurnExpectation`
    - `wait_for_customer_tracking_catchup_without_error(...)`
- The exact names may change during execution, but the boundary must remain customer-specific and typed.

## Public Contract To Establish

- One ignored long-lane test proves high source write churn end to end:
  - bootstrap the default customers migration through the real bootstrap script and real runner
  - capture a baseline customer tracking snapshot
  - execute a deterministic burst of customer inserts, updates, deletes, and re-inserts through real Cockroach SQL after CDC setup is complete
  - use a bounded id range and deterministic final state so the test can assert exact helper-shadow and real-table snapshots
  - wait for the helper shadow customers to converge to the final expected snapshot
  - wait for the real destination customers to converge to the same final expected snapshot
  - assert tracking progress has advanced beyond baseline, catches up through the received watermark, and leaves `last_error` empty
  - assert the runner stayed alive through the churn workload
  - run `runner verify`
  - record an explicit task verdict:
    - if the lane passes honestly, the task markdown should say the current design handled this bounded churn workload acceptably and no concrete design change was required
    - if the lane exposes instability, the task markdown should record the concrete failure and the specific improvement need instead of claiming success

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - customer-specific churn orchestration does not belong in `default_bootstrap_long_lane.rs`
- Required cleanup during execution:
  - move churn SQL and final expected-snapshot modeling behind one customer-specific helper boundary
  - keep generic polling and raw SQL execution in `e2e_harness.rs`
  - keep churn verdict wording in task bookkeeping, not inside product code or test-only log parsing
- Smells to avoid:
  - pushing a large inline SQL script and multiple ad hoc snapshot strings directly into the long-lane test
  - introducing a fake-generic mutation framework in the generic harness for one customer-only scenario
  - relying on sleeps alone instead of observable helper-shadow, destination, and tracking-state convergence

## Files And Structure To Add Or Change

- [x] `crates/runner/tests/support/default_bootstrap_harness.rs`
  - add one typed customer churn workload helper that owns the mutation burst and its final expected snapshot
  - add one customer-specific catch-up helper if the test body would otherwise duplicate tracking-watermark logic
- [x] `crates/runner/tests/default_bootstrap_long_lane.rs`
  - add the ignored high-source-write-churn scenario
- [x] Product code changes are not expected
  - only real bug fixes are allowed if the RED slices expose one
  - likely hotspots only if a real correctness bug appears:
    - `crates/runner/src/webhook_runtime/persistence.rs`
    - `crates/runner/src/reconcile_runtime/mod.rs`
    - `crates/runner/src/tracking_state.rs`

## TDD Execution Order

### Slice 1: Tracer Bullet For One Deterministic High-Churn Workload

- [x] RED: add one ignored failing long-lane test that bootstraps the default customers migration, runs a deterministic burst of repeated create/update/delete customer mutations, and expects the real destination customers to converge to a known final snapshot
- [x] GREEN: add only the minimum customer-specific churn helper support needed to drive the real workload and final destination assertion honestly
- [x] REFACTOR: keep churn workload construction in `default_bootstrap_harness.rs`, not inline in the long-lane test

### Slice 2: Prove Helper Shadow Convergence Under Churn

- [x] RED: strengthen the same scenario to assert the helper shadow customers converge to the exact same final snapshot as the real destination table
- [x] GREEN: the existing public-path behavior already satisfied the stronger helper-shadow assertion without product changes
- [x] REFACTOR: keep expected-snapshot rendering in one customer-specific support boundary instead of duplicating snapshot strings across assertions

### Slice 3: Prove Tracking Catch-Up Without Durable Error

- [x] RED: strengthen the scenario to assert:
  - the received watermark advances beyond baseline during the churn workload
  - the reconciled watermark and table sync watermark eventually catch up through the received watermark
  - `table_sync_state.last_error` remains empty after convergence
- [x] GREEN: the existing tracking helpers plus typed churn expectation were enough to express and pass the stronger behavior honestly
- [x] REFACTOR: keep generic tracking polling in `e2e_harness.rs` and only wrap customer-specific intent in `default_bootstrap_harness.rs`

### Slice 4: Prove No Silent Runner Failure And Record The Design Verdict

- [x] RED: strengthen the scenario to assert the runner stayed alive throughout the churn window and the final task bookkeeping records whether the current design handled the churn acceptably or exposed a concrete improvement need
- [x] GREEN: add only the minimal generic runner-liveness support needed for an explicit assertion, and record the bookkeeping verdict in the task markdown
- [x] REFACTOR: do not add product-side test hooks or stringly stderr scraping when existing runner-process assertions are sufficient

### Slice 5: Real Verify And No Shortcut Regression

- [x] RED: run `runner verify` after convergence and assert the output reports a matched verdict for the real migrated table only
- [x] GREEN: keep verification at the CLI boundary and do not add any alternate verification path
- [x] REFACTOR: confirm no product file gained a churn-only branch or hidden shortcut

### Slice 6: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so customer-specific churn orchestration, generic harness plumbing, and long-lane behavior assertions each own one clear responsibility

## TDD Guardrails For Execution

- Every new behavior assertion must fail before code changes are added. If an assertion already passes, replace it with the next uncovered behavior.
- Do not fake high churn with direct helper-table writes, fake webhook payloads, or hidden runner hooks.
- Do not use blind sleeps as the main proof of correctness. Prefer helper-shadow polling, destination polling, runner-liveness checks, and tracking-state polling.
- Do not weaken the workload into one or two trivial updates. The workload must include repeated creates, updates, deletes, and at least one re-insert or replacement pattern so the final state is not a trivial last-write-only case.
- Do not swallow source SQL, runner, Docker, verify, or polling errors. Any such failure is a real test failure.
- If the churn workload reveals a real correctness bug, fix the bug rather than diluting the assertions.

## Boundary Review Checklist

- [x] Customer-specific churn scripting lives in `default_bootstrap_harness.rs`, not in the long-lane test body
- [x] Generic SQL execution and tracking polling remain in `e2e_harness.rs`
- [x] Final state is asserted through exact helper-shadow and real-table snapshots, not by loose row counts alone
- [x] Tracking progress proves catch-up and no durable error after the churn settles
- [x] Product code contains no churn-only test hooks
- [x] No error path is swallowed or silently ignored

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes, add the explicit churn verdict note, and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
