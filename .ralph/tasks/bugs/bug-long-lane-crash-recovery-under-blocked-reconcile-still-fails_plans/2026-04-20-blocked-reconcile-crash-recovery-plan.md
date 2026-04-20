# Plan: Stabilize Blocked-Reconcile Runner Crash Recovery Around The Real Tracking Contract

## References

- Task:
  - `.ralph/tasks/bugs/bug-long-lane-crash-recovery-under-blocked-reconcile-still-fails.md`
- Failing long-lane coverage:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Shared E2E support:
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
- Runtime and tracking behavior:
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/tracking_state.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn is planning-only because the bug task had no `<plan>` pointer and no execution marker.
- The failure is currently isolated to the ignored long-lane test
  - `ignored_long_lane_recovers_after_runner_crash_during_a_blocked_reconcile_pass`
- The bug may be one of two things:
  - a real runtime recovery defect
  - a brittle harness/test expectation that over-specifies internal timing around failure persistence
- `tracking_state` already makes one important contract explicit:
  - `persist_reconcile_failure()` stores `last_error`
  - `persist_reconcile_success()` clears `last_error` and advances the reconciled watermark
- If the first RED slice proves the current pre-crash state cannot be expressed honestly without depending on racey internal timing, execution must move the test to a typed shared audit and stop asserting raw field combinations directly from the long-lane file.
- If the first RED slice proves the runtime actually loses received watermarks, fails to recover the blocked change after restart, or leaves stale failure state after success, execution must fix runtime behavior instead of weakening the test.

## Current State Summary

- The failing test currently does this:
  - bootstraps the default migration
  - locks the destination `customers` table
  - applies a source update
  - waits for helper persistence to succeed
  - waits for tracking state that says:
    - received watermark advanced
    - reconciled watermark stayed at baseline
    - last successful sync watermark stayed at baseline
    - `last_error.is_none()`
  - kills the runner
  - releases the lock
  - restarts the runner
  - expects verify correctness plus tracking recovery through the received watermark
- The likely brittle seam is the pre-crash `last_error.is_none()` assertion.
  - `reconcile_runtime::run_reconcile_pass()` records a reconcile failure after rollback and before the next pass.
  - If the runner reaches that persistence step before the test kills it, `last_error` will be present even though the recovery contract may still be correct.
- That means the current long-lane test is mixing two layers:
  - the operator-visible recovery contract we actually care about
  - an internal timing guess about whether failure tracking commits before process death

## Execution Update

- The blocked-reconcile scenario no longer reproduces as the suite blocker in the current repo state.
  - Focused run passed:
    - `cargo test -p runner ignored_long_lane_recovers_after_runner_crash_during_a_blocked_reconcile_pass --test default_bootstrap_long_lane -- --ignored --exact --nocapture`
- The real current long-lane failure is elsewhere:
  - `ignored_long_lane_recovers_after_reconcile_transaction_failure`
- The full ignored lane currently fails because the harness still expects a hard process crash:
  - `DefaultBootstrapHarness::wait_for_runner_failed_exit()`
  - `RunnerProcess::wait_for_failed_exit()`
- Observed failure:
  - `runner did not exit with failure in time`
  - stderr repeatedly logs `failed to apply reconcile upsert for mapping \`app-a\` in \`app_a\` real table \`public.customers\`: error returned from database: forced destination write failure`
- That means the active design assumption in this plan is wrong.
  - The runner appears to stay alive and keep retrying reconcile transaction failures instead of terminating.
  - Future execution must re-scope around the real contract for reconcile-transaction failure recovery before changing test or runtime code.

## Improve-Code-Boundaries Focus

- Primary smell:
  - `default_bootstrap_long_lane.rs` owns raw tracking-state predicates that belong in shared E2E support or typed integrity audits.
- Secondary smell:
  - blocked-reconcile recovery semantics are encoded as inline boolean expressions over `MappingTrackingProgress`, which makes the long-lane file responsible for low-level state-machine timing.
- Preferred cleanup direction:
  - move blocked-reconcile crash-recovery classification into shared support
  - expose one typed audit from `DefaultBootstrapHarness`
  - keep the long-lane test responsible only for scenario setup and the final operator-visible assertions
- Bold refactor allowance:
  - if a typed blocked-reconcile recovery audit can replace multiple ad hoc inline predicates, prefer deleting those inline predicates rather than wrapping them in more helpers

## Intended Public Contract After Execution

- The blocked-reconcile crash-recovery scenario must prove these operator-visible guarantees:
  - the new source change is durably received before the crash
  - reconcile has not yet advanced through that watermark before the crash
  - after restart and lock release, the runner reconciles through the previously received watermark
  - verify correctness converges after restart
  - `last_error` is cleared once recovery succeeds
- The test must not depend on a racey internal distinction between:
  - “runner was killed before failure tracking committed”
  - “runner recorded a reconcile failure just before being killed”
- A pre-crash persisted reconcile failure is acceptable only if:
  - the received watermark stays monotonic
  - the reconciled watermark remains behind the received watermark before restart
  - restart still catches up cleanly and clears the error on success

## Expected Code Shape

- `crates/runner/tests/support/e2e_integrity.rs`
  - add a typed audit for blocked-reconcile crash recovery
  - keep allowed pre-crash states and required post-restart states here instead of scattering boolean logic
- `crates/runner/tests/support/default_bootstrap_harness.rs`
  - add one named scenario helper that:
    - sets up the blocked reconcile
    - captures pre-crash progress
    - restarts the runner after lock release
    - returns the typed audit
- `crates/runner/tests/default_bootstrap_long_lane.rs`
  - rewrite the failing long-lane test to use the new typed audit instead of poking `MappingTrackingProgress` directly
- Runtime files only if the RED slice proves a real defect:
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/tracking_state.rs`

## Type And Boundary Decisions

- Preferred typed audit shape:
  - `BlockedReconcileCrashRecoveryAudit`
- The audit should own enough evidence to assert:
  - pre-crash received watermark advanced beyond baseline
  - pre-crash reconciled watermark stayed at the last good checkpoint
  - pre-crash state optionally recorded a reconcile failure
  - post-restart state reconciled through the previously received watermark
  - post-restart state cleared `last_error`
  - verify-image correctness converged after restart
- Keep `MappingTrackingProgress` as a low-level snapshot type.
  - Do not keep teaching the long-lane file how to interpret every field combination.
- Do not add:
  - sleeps or retries whose only purpose is to force `last_error` into one exact transient state
  - log-scraping-only assertions when the tracking tables already carry the durable contract
  - duplicate blocked-reconcile helpers in both the long-lane file and the harness

## Vertical TDD Slices

### Slice 1: Tracer Bullet For The Real Contract

- [ ] RED: add one failing integration-style test or focused contract assertion that captures the real blocked-reconcile recovery bug without relying on raw inline state predicates from the long-lane file
- [ ] GREEN: implement the smallest shared audit/helper needed so the scenario is expressed through the typed contract
- [ ] REFACTOR: delete the old inline pre-crash predicate if the new audit fully owns it
- Stop condition:
  - if this slice proves the desired contract itself is unclear, switch this plan back to `TO BE VERIFIED` and stop immediately

### Slice 2: Decide Whether The Failure Is Harness Or Runtime

- [ ] RED: rerun the focused ignored long-lane scenario and capture the exact failing state or assertion
- [ ] GREEN:
  - if the failure is only the brittle `last_error.is_none()` assumption, fix the harness/audit contract
  - if the failure is a real runtime defect, fix reconcile/tracking behavior instead
- [ ] REFACTOR: keep failure-classification logic inside the typed audit, not in the test body

### Slice 3: Prove Recovery End-To-End

- [ ] RED: tighten the blocked-reconcile crash-recovery assertions so they require:
  - monotonic received watermark
  - stalled reconciled watermark before restart
  - catch-up through the blocked watermark after restart
  - cleared `last_error` after successful recovery
- [ ] GREEN: make the minimum runtime or harness change needed to satisfy those assertions
- [ ] REFACTOR: remove any dead helper or duplicate progress interpretation left behind

### Slice 4: Boundary Audit

- [ ] Run one final `improve-code-boundaries` sweep with this question:
  - does `default_bootstrap_long_lane.rs` still contain raw tracking-state interpretation that should live in shared support?
- [ ] If yes, move it now rather than preserving another fragile inline state machine

### Slice 5: Repository Validation

- [ ] Run the focused ignored test until it passes deterministically
- [ ] Run `make check`
- [ ] Run `make lint`
- [ ] Run `make test`
- [ ] Run `make test-long`

## Expected Outcome

- The long-lane scenario should stop failing because it will assert the real operator-visible recovery contract instead of a racey pre-crash timing detail.
- If the runtime is actually wrong, the RED slice should expose that cleanly and the fix should land in reconcile/tracking code, not in test sleeps.
- The blocked-reconcile crash-recovery behavior should be owned by one deeper shared audit boundary rather than one ad hoc long-lane predicate.

Plan path: `.ralph/tasks/bugs/bug-long-lane-crash-recovery-under-blocked-reconcile-still-fails_plans/2026-04-20-blocked-reconcile-crash-recovery-plan.md`

TO BE VERIFIED
