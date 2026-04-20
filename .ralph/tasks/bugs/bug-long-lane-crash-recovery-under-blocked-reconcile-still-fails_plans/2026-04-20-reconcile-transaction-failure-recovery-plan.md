# Plan: Repair Reconcile Transaction Failure Long-Lane Coverage Around The Live-Retry Contract

## References

- Task:
  - `.ralph/tasks/bugs/bug-long-lane-crash-recovery-under-blocked-reconcile-still-fails.md`
- Failing long-lane coverage:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Shared E2E support:
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
  - `crates/runner/tests/support/runner_process.rs`
- Runtime and tracking behavior:
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/tracking_state.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Why The Previous Plan Is Wrong

- The current suite blocker is not the blocked-reconcile crash test anymore.
  - Focused blocked-reconcile crash recovery already passed in isolation.
- The real failing test is:
  - `ignored_long_lane_recovers_after_reconcile_transaction_failure`
- The stale assumption is in the test/harness boundary:
  - it still calls `wait_for_runner_failed_exit()`
  - it assumes destination reconcile failure should terminate the runner process
- The runtime contract disagrees with that assumption:
  - `run_mapping_loop()` keeps ticking forever
  - `run_reconcile_pass()` rolls back the failed transaction
  - `persist_reconcile_failure()` stores operator-visible `last_error`
  - the loop returns `Ok(ReconcilePassOutcome::ApplyFailedRecorded)` and retries later
- So the active defect is very likely a boundary/design problem in test support, not a runner crash-recovery bug.

## TDD Contract For The Next Turn

- Use vertical Red-Green TDD only.
- One behavior at a time:
  - write one failing test
  - make that one test pass
  - rerun and reassess before the next slice
- Tests must assert operator-visible behavior through public harness/audit interfaces, not raw process or internal timing trivia.

## Improve-Code-Boundaries Focus

- Primary smell:
  - `default_bootstrap_long_lane.rs` directly interprets low-level tracking state and process lifecycle instead of calling one scenario-specific audit.
- Secondary smell:
  - `wait_for_runner_failed_exit()` is a reusable harness primitive, but this scenario should not own a process-exit contract at all because runtime semantics are retry-in-place.
- Cleanup direction:
  - add one typed audit for reconcile transaction failure and recovery
  - move progress classification and stderr expectations into shared support
  - keep the long-lane file focused on scenario intent, not mapping-state mechanics
- Bold refactor allowance:
  - if the new typed audit fully subsumes the old inline assertions, delete those inline assertions instead of wrapping them again

## Intended Public Contract

- When a destination reconcile write fails:
  - the helper shadow state still advances with the new source row
  - verify correctness exposes a selected-table mismatch while destination is blocked by the forced write failure
  - tracking state records that the new watermark was received
  - reconciled and last-successful watermarks stay at the last good checkpoint
  - `last_error` persists operator-visible reconcile failure context
  - the runner remains alive and keeps retrying rather than crashing
- After the write failure is removed:
  - the same runner instance or a restarted runner converges without manual state repair
  - reconcile catches up through the previously received watermark
  - `last_error` is cleared on successful recovery
  - verify correctness returns to selected-table match

## Expected Code Shape

- `crates/runner/tests/support/e2e_integrity.rs`
  - add a typed audit such as `ReconcileTransactionFailureAudit`
  - encode failure classification and recovery assertions there
- `crates/runner/tests/support/default_bootstrap_harness.rs`
  - add one scenario helper that:
    - establishes baseline progress
    - injects destination write failure
    - waits for helper shadow persistence
    - captures the failed reconcile state while runner stays alive
    - releases the failure and waits for recovery
    - returns the typed audit
- `crates/runner/tests/default_bootstrap_long_lane.rs`
  - replace the inline failure/exit/recovery assertions with the new audit call
- Runtime files:
  - change only if the first RED slice proves the runner is not actually honoring the retry-and-recover contract

## Type Decisions

- Preferred audit type:
  - `ReconcileTransactionFailureAudit`
- The audit should own enough evidence to assert:
  - helper shadow snapshot after ingress persistence
  - verify mismatch during reconcile failure
  - failure progress stalled at the baseline reconcile watermark
  - persisted `last_error` contains reconcile failure context
  - runner stayed alive during failure
  - stderr contains operator-visible reconcile failure logs
  - recovery reconciled through the failed watermark
  - recovery cleared `last_error`
  - verify correctness matched again after recovery
- Keep `MappingTrackingProgress` as the low-level raw snapshot.
  - The long-lane test should not keep teaching itself how to interpret every field combination.

## Vertical TDD Slices

### Slice 1: Replace The Wrong Exit Contract

- [x] RED: rewrite the failing long-lane scenario to assert the real contract first
  - destination reconcile failure does not kill the runner
  - a persisted mismatch plus stalled reconcile watermark is observable
  - this should fail because the typed audit/helper does not exist yet
- [x] GREEN: add the minimum harness/integrity support needed to express that scenario through a typed audit
- [x] REFACTOR: remove `wait_for_runner_failed_exit()` from this scenario if the typed audit fully replaces it
- Stop condition:
  - if evidence shows the runner actually should exit on reconcile apply failure, switch the plan back to `TO BE VERIFIED` and stop immediately

### Slice 2: Prove Failure Classification Cleanly

- [x] RED: tighten the audit so it requires:
  - helper shadow advanced to the failed value
  - verify image reports selected-table mismatch
  - received watermark advanced beyond baseline
  - reconciled watermark remained at the baseline checkpoint
  - `last_error` persisted reconcile context
  - runner remained alive
- [x] GREEN: implement the smallest additional audit data gathering or harness waits needed
- [x] REFACTOR: keep all state-machine interpretation in `e2e_integrity.rs`, not in test bodies

### Slice 3: Prove Recovery After Failure Removal

- [x] RED: extend the same audit to require post-failure convergence
  - drop the injected write failure
  - verify image returns to match
  - tracking progress reconciles through the failed received watermark
  - `last_error` becomes `None`
- [x] GREEN: implement only the minimum recovery wait/assertion path needed
- [x] REFACTOR: delete any duplicate recovery predicate left inline in the long-lane file

### Slice 4: Boundary Sweep

- [x] Run one explicit `improve-code-boundaries` sweep on the touched files
- [x] Check whether `default_bootstrap_long_lane.rs` still contains raw tracking-state or process-lifecycle interpretation that should live in the typed audit
- [x] If yes, move it before final validation

### Slice 5: Required Validation

- [x] Run the focused ignored long-lane test until it passes deterministically
- [x] Run `make check`
- [x] Run `make lint`
- [x] Run `make test`
- [x] Run `make test-long`

## Expected Outcome

- The failing long-lane scenario should stop asserting a nonexistent crash contract.
- Reconcile transaction failure should be expressed as a bounded live-retry scenario owned by one deeper harness/audit boundary.
- If runtime behavior is actually defective, the first RED slice should expose that clearly without muddying the test with process-exit assumptions.

Plan path: `.ralph/tasks/bugs/bug-long-lane-crash-recovery-under-blocked-reconcile-still-fails_plans/2026-04-20-reconcile-transaction-failure-recovery-plan.md`

NOW EXECUTE
