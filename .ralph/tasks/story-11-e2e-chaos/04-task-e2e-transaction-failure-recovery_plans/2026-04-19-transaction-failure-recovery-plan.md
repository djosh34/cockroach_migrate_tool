# Plan: End-To-End Destination Transaction Failure Recovery

## References

- Task: `.ralph/tasks/story-11-e2e-chaos/04-task-e2e-transaction-failure-recovery.md`
- Previous story-11 execution plans:
  - `.ralph/tasks/story-11-e2e-chaos/02-task-e2e-receiver-crash-and-restart-recovery_plans/2026-04-19-receiver-crash-restart-recovery-plan.md`
  - `.ralph/tasks/story-11-e2e-chaos/03-task-e2e-network-fault-injection-imposed-externally_plans/2026-04-19-external-network-fault-e2e-plan.md`
- Existing long-lane scenarios:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Existing long-lane support:
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/runner_process.rs`
  - `crates/runner/tests/support/webhook_chaos_gateway.rs`
  - `crates/runner/tests/support/destination_lock.rs`
- Runtime surfaces involved in destination-side transactions:
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/reconcile_runtime/upsert.rs`
  - `crates/runner/src/tracking_state.rs`
- Existing failure-state coverage to preserve and extend:
  - `crates/runner/tests/reconcile_contract.rs`
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
- The design doc explicitly defines the helper-persistence contract:
  - if the destination receiver transaction fails, the receiver returns non-`200`
  - Cockroach retries
- The current reconcile runtime returns an error on a failed reconcile pass, which terminates `runner run`; the recovery path for reconcile transaction failure therefore includes an honest runner restart after the destination fault is removed.
- No product failpoint, hidden test-only branch, alternate endpoint, or fake ingest path may be added.
- The honest way to force a destination transaction failure is destination-side SQL behavior owned by test support:
  - failing trigger or equivalent DDL on the helper shadow table for webhook persistence failure
  - failing trigger or equivalent DDL on the real target table for reconcile failure
- For the helper-persistence scenario, the existing external HTTPS gateway may be used as an observer only:
  - it forwards real requests to the runner
  - it records downstream statuses
  - it does not inject the failure itself
- If a trigger-based destination failure cannot be installed, scoped to the target row, and cleaned up deterministically from test support, this plan must be switched back to `TO BE VERIFIED` before execution continues.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - a failed helper persistence transaction returns a non-`200` response and the same row is retried until it succeeds
  - helper persistence failure does not partially commit helper shadow state or silently skip work
  - a failed reconcile transaction records durable failure state without falsely advancing the reconciled watermark
  - after the destination fault is removed, the system recovers and the real target tables converge correctly
  - `runner verify` still passes against the real migrated table only
- Lower-priority implementation concerns:
  - keep long-lane tests readable
  - keep destination-fault orchestration and gateway observation in generic support, not inline SQL or stringly log parsing in the test body

## Interface And Boundary Decisions

- Keep all product CLIs unchanged.
- Add one dedicated destination-side failure support boundary under `crates/runner/tests/support/`:
  - suggested name: `destination_write_failure.rs`
  - responsibility:
    - install a deterministic failing SQL trigger or equivalent database-side fault on one qualified table
    - scope the failure to the targeted row payload so bootstrap traffic does not trip it accidentally
    - remove the fault deterministically on drop
- Keep `CdcE2eHarness` responsible for:
  - Docker lifecycle
  - config materialization
  - source and destination SQL helpers
  - runner startup and restart
  - tracking-state polling
  - generic gateway observation helpers
- Flatten one current wrong boundary in `webhook_chaos_gateway.rs`:
  - today the support API mostly exposes injected-fault-specific assertions
  - this task needs typed observation of forwarded downstream statuses even when the gateway injects nothing
  - execution should generalize outcome inspection so helper-persistence retry assertions do not pretend a gateway fault happened
- Flatten one naming smell in `default_bootstrap_harness.rs` if needed:
  - the current constructor `start_with_external_sink_faults()` is too fault-specific for a scenario that uses the gateway only as an observer
  - prefer a name that reflects the actual boundary, such as an external observable gateway, and keep fault-arming methods separate
- Add one explicit runner-exit support helper if needed:
  - expected reconcile failure is not the same as an accidental crash
  - tests should be able to wait for and assert the deliberate runner exit context instead of relying on `assert_runner_alive()` panics
- Keep customer-specific wrappers and snapshot assertions in `default_bootstrap_harness.rs`.

## Public Contract To Establish

- One ignored long-lane test proves helper-persistence transaction failure and retry end to end:
  - bootstrap the default customers migration
  - install a destination-side helper-table write failure for one live customer-email update
  - perform the source update
  - observe repeated runner `500` responses for the same forwarded payload while the failure is active
  - confirm helper shadow state and real destination state do not falsely advance
  - remove the destination failure
  - observe a later successful retry of that same payload
  - confirm helper shadow and real destination tables converge
  - run `runner verify`
- One ignored long-lane test proves reconcile transaction failure and recovery end to end:
  - bootstrap the default customers migration
  - install a destination-side real-table write failure for one live customer-email update
  - perform the source update
  - wait until helper shadow state has durably advanced
  - observe reconcile failure state in `_cockroach_migration_tool.table_sync_state`
  - confirm the reconciled watermark and last successful table watermark do not falsely advance
  - assert the runner exits because reconcile returned a real runtime error
  - remove the destination failure
  - restart the runner
  - confirm the real destination table converges
  - confirm the recorded reconcile error clears after the successful retry
  - run `runner verify`

## Vertical TDD Slices

### Slice 1: Tracer Bullet For Helper-Persistence Transaction Failure

- [ ] RED: add one ignored failing long-lane test that installs a helper-table destination write failure for one customer email update, performs the update through the real Cockroach changefeed path, then expects eventual convergence only after the destination failure is removed
- [ ] GREEN: add only the minimum destination-failure support and gateway-observation support needed for that scenario to pass honestly
- [ ] REFACTOR: keep destination-side fault installation in dedicated support instead of raw DDL inside the long-lane test

### Slice 2: Prove Retry And No Partial Helper Commit

- [ ] RED: strengthen the helper-persistence scenario to assert:
  - the gateway observed the same forwarded payload receive downstream `500` before later receiving downstream `200`
  - helper shadow state and real destination state stayed at the old value while the destination failure remained active
  - after release, helper shadow state and the real destination table both converged to the updated value
- [ ] GREEN: generalize gateway outcome inspection only as far as needed to assert status-sequence behavior without pretending the gateway injected the failure
- [ ] REFACTOR: keep status-sequence matching and attempt summaries inside gateway support, not in the test body

### Slice 3: Tracer Bullet For Reconcile Transaction Failure

- [ ] RED: add one ignored failing long-lane test that installs a real-table destination write failure, performs a live source update, waits until helper shadow state has advanced, and then expects the reconcile pass to fail and the runner to exit
- [ ] GREEN: add only the minimum destination-failure support and expected-runner-exit support needed to make that failure observable through public behavior
- [ ] REFACTOR: keep deliberate-exit waiting in `runner_process.rs` or generic harness support rather than scattering process polling logic in the scenario

### Slice 4: Prove Durable Failure Tracking And Recovery

- [ ] RED: strengthen the reconcile-failure scenario to assert:
  - `table_sync_state.last_error` records the real reconcile failure context
  - `latest_reconciled_resolved_watermark` and `last_successful_sync_watermark` stay at the last good checkpoint while the new helper state is already present
  - after the destination fault is removed and the runner restarts, the destination table converges and `last_error` clears
- [ ] GREEN: fix only the first real correctness bug exposed by the durable-tracking assertions
- [ ] REFACTOR: keep tracking snapshot parsing and watermark comparisons behind typed harness helpers, not copied SQL in the test body

### Slice 5: Real Verify And No Shortcut Regression

- [ ] RED: run `runner verify` after each recovery scenario and assert the output reports a matched verdict for the real migrated table only
- [ ] GREEN: keep verification at the CLI boundary and do not add any alternate verification path
- [ ] REFACTOR: confirm no product file gained a transaction-failure-only branch or hidden retry hook

### Slice 6: Full Repository Lanes And Final Boundary Review

- [ ] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [ ] GREEN: continue until every required lane passes cleanly
- [ ] REFACTOR: do one final `improve-code-boundaries` pass so destination write failures, runner lifecycle, gateway observation, generic harness setup, and customer scenario assertions each own one clear responsibility

## TDD Guardrails For Execution

- Every new behavior assertion must fail before code changes are added. If an assertion already passes, replace it with the next uncovered behavior.
- Do not use blind sleeps to guess retry or recovery. Prefer gateway-observed statuses, helper-shadow polling, destination polling, and tracking-state polling.
- Do not model destination transaction failure with a gateway fault, in-binary failpoint, or fake response body. The failure must come from PostgreSQL behavior on the destination side.
- Do not run extra source-side shell commands or manual repair SQL after CDC setup is complete.
- Do not swallow trigger-installation, SQL, runner, Docker, gateway, restart, or verify errors. Any such failure is a real test failure.
- If a new support type only wraps one trivial call path, inline it instead of growing the support tree for no gain.

## Boundary Review Checklist

- [ ] Destination-side transaction failure is installed through typed test support, not raw SQL embedded in long-lane tests
- [ ] Gateway observation can describe forwarded status sequences without conflating observation with injected faults
- [ ] Runner lifecycle support can distinguish expected reconcile-failure exit from accidental early exit
- [ ] Customer-specific assertions stay in `default_bootstrap_harness.rs`
- [ ] Product code contains no destination-transaction-failure test hooks
- [ ] No error path is swallowed or silently ignored

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long`
- [ ] One final `improve-code-boundaries` pass after all lanes are green
- [ ] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
