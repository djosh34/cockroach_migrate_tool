# Plan: Track Reconciled Watermarks And Repeatable Sync State

## References

- Task: `.ralph/tasks/story-07-reconcile/03-task-track-reconciled-watermarks-and-repeatable-sync-state.md`
- Previous task plan: `.ralph/tasks/story-07-reconcile/02-task-build-continuous-delete-reconcile-pass_plans/2026-04-18-continuous-delete-reconcile-plan.md`
- Related task plan: `.ralph/tasks/story-06-destination-ingest/03-task-persist-resolved-watermarks-and-stream-state_plans/2026-04-18-resolved-watermark-and-stream-state-plan.md`
- Design: `designs/crdb-to-postgres-cdc/02_requirements.md`
- Design: `designs/crdb-to-postgres-cdc/03_shadow_table_architecture.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Current implementation:
  - `crates/runner/src/postgres_bootstrap.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/reconcile_runtime/upsert.rs`
  - `crates/runner/src/reconcile_runtime/delete.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/tracking_state.rs`
  - `crates/runner/src/error.rs`
- Current tests:
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- This task stays inside the existing destination-side contract:
  - `runner run --config <path>` remains the only production entrypoint
  - the reconcile worker keeps using helper shadow tables as truth
  - no new source-bootstrap command, source cursor handoff, or cutover API is added here
- The tracking schema stays intentionally small. This task should finish the meaning of the existing state, not invent a second control-plane model:
  - `_cockroach_migration_tool.stream_state`
  - `_cockroach_migration_tool.table_sync_state`
- "Caught up enough for drain-to-zero visibility" is defined from existing fields rather than a new readiness flag:
  - `latest_received_resolved_watermark` is the helper-shadow frontier
  - `latest_reconciled_resolved_watermark` is the last fully successful real-table frontier
  - each table is caught up only when `last_successful_sync_watermark` matches the latest reconciled frontier and `last_error` is `NULL`
- Reconcile failure must stay loud. A failed pass may still terminate the process, but it must persist failure state before surfacing the error so restart-and-resume is inspectable.
- Restart/resume in scope means destination-side repeatability:
  - bootstrap reruns must preserve prior success and error state
  - restarting after a failed or partial run must let the next run continue from helper-shadow truth without manual cleanup of tracking rows
- If the first execution slice proves that failure state cannot be persisted without introducing a second history table or a much richer state machine, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Flatten the current wrong-place boundary from `improve-code-boundaries`:
  - `bootstrap_postgres` seeds tracking state
  - `webhook_runtime` persists resolved watermarks
  - `reconcile_runtime` persists success state
  - yet all three currently depend on `webhook_runtime::tracking`
  - task 03 should move tracking ownership into a shared top-level module such as `tracking_state.rs`
- Reconcile runtime should stop knowing SQL details of tracking persistence:
  - `reconcile_runtime` runs one pass and produces a typed pass outcome
  - the tracking module records either success or failure
  - `reconcile_runtime` remains orchestration-only after the pass result is known
- The tracking boundary should own one typed outcome model for reconcile:
  - success: mapping id, database, reconciled watermark, tables whose sync state advances
  - failure: mapping id, database, failing table, phase (`upsert` or `delete`), and rendered error text
- Keep the state model small and explicit:
  - do not add a new "cutover ready" table
  - do not add per-pass history rows
  - use existing columns plus minimal `stream_status` transitions only if they clarify current state without creating a fake workflow engine
- Failure semantics should be merge-based, not destructive:
  - a failed pass must not advance `latest_reconciled_resolved_watermark`
  - a failed pass must not rewrite `last_successful_sync_watermark`
  - the failing table should gain a loud `last_error`
  - a later successful full pass should clear stale `last_error` values for the mapped tables it just reconciled successfully
- Bootstrap seeding stays idempotent and preserve-first:
  - `source_database` and helper-table names may be refreshed from config/helper metadata
  - success watermarks, reconciled watermarks, and last errors must survive restart

## Public Contract To Establish

- Successful reconcile passes continue to advance `latest_reconciled_resolved_watermark` only after the full upsert-plus-delete pass commits.
- Per-table sync state remains sufficient to tell whether real tables are caught up to helper state:
  - each table keeps its last successful sync watermark
  - failed tables keep a visible `last_error`
  - operators can tell "helper state is ahead of real state" by comparing latest received vs latest reconciled and inspecting table rows
- Reconcile failures become durable and restart-visible:
  - the process does not swallow the failure
  - the failure is written to tracking state before the error is returned
  - restart does not erase that failure evidence
- A successful rerun after a prior failure clears the stale error and advances both stream-level and table-level progress.
- Replaying or retrying after restart remains safe:
  - helper truth stays the source of reconciliation
  - resolved frontiers remain monotonic
  - older or duplicate helper-side progress does not move tracked success backward

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this task:
  - tracking state lives in `webhook_runtime::tracking.rs`, but bootstrap and reconcile both depend on it
  - that makes the webhook module the courier for unrelated runtime internals
  - task 03 should extract a shared tracking module and remove the `reconcile_runtime -> webhook_runtime` dependency
- Required cleanup:
  - move seed, resolved, reconcile-success, and reconcile-failure persistence behind one neutral tracking boundary
  - replace ad hoc tracking calls from reconcile with one typed pass-result API
  - render `last_error` text in one place from typed failure context instead of scattering string formatting across runtime code
- Secondary cleanup:
  - keep bootstrap as DDL/schema discovery only; avoid growing more tracking SQL there once the shared module exists
  - keep `reconcile_runtime/mod.rs` focused on interval orchestration and transaction control, not state-merge rules
  - avoid introducing duplicated mapping/runtime shapes just to feed tracking persistence

## Files And Structure To Add Or Change

- [x] `crates/runner/src/lib.rs`
  - register the shared tracking module and remove any now-wrong module ownership
- [x] `crates/runner/src/tracking_state.rs`
  - new neutral tracking boundary for seed, resolved, reconcile-success, and reconcile-failure persistence
- [x] `crates/runner/src/webhook_runtime/tracking.rs`
  - delete or fully replace this file so tracking no longer lives under the webhook namespace
- [x] `crates/runner/src/postgres_bootstrap.rs`
  - call the shared seed/merge API and keep restart behavior preserve-first
- [x] `crates/runner/src/reconcile_runtime/mod.rs`
  - return typed pass outcomes, rollback failed data transactions, then persist failure or success through the shared tracking boundary
- [x] `crates/runner/src/reconcile_runtime/upsert.rs`
  - preserve typed table/phase context needed for failure recording without duplicating tracking logic
- [x] `crates/runner/src/reconcile_runtime/delete.rs`
  - same as upsert: keep apply logic local while exposing enough failure context for tracking
- [x] `crates/runner/src/error.rs`
  - add explicit failure-state persistence errors if the current tracking-update errors no longer distinguish success-path and failure-path writes
- [x] `crates/runner/tests/reconcile_contract.rs`
  - add end-to-end contract coverage for failure tracking, preserved frontiers, successful retry, and restart-safe resume
- [x] `crates/runner/tests/bootstrap_contract.rs`
  - extend restart coverage so bootstrap reruns preserve reconcile failure/success evidence, not only seeded helper metadata
- [x] `crates/runner/tests/webhook_contract.rs`
  - keep resolved-watermark monotonicity and restart behavior green after the tracking boundary moves

## TDD Execution Order

### Slice 1: Tracer Bullet For Durable Reconcile Failure State

- [x] RED: add one failing reconcile contract test that creates a real-table constraint violation reachable only during reconcile, posts a newer resolved watermark, and expects the runner to fail loudly while leaving `latest_reconciled_resolved_watermark` unchanged and recording `last_error` for the failing table
- [x] GREEN: implement the smallest reconcile-failure persistence path that records the failure after rollback and before surfacing the runtime error
- [x] REFACTOR: introduce a shared tracking module plus a typed `ReconcilePassFailure` boundary so reconcile no longer depends on `webhook_runtime::tracking`

### Slice 2: Successful Retry Clears The Error And Advances Progress

- [x] RED: add failing coverage that restarts the runner after the recorded failure, sends corrected helper truth for the same table, and expects a later successful pass to clear `last_error`, advance `latest_reconciled_resolved_watermark`, and update `last_successful_sync_watermark`
- [x] GREEN: implement success/failure merge rules so prior failure evidence survives until a real successful pass replaces it
- [x] REFACTOR: keep all tracking-state merge rules inside the shared tracking boundary instead of mixing them into bootstrap or reconcile orchestration

### Slice 3: Multi-Table Catch-Up Visibility

- [x] RED: add failing coverage for a multi-table mapping where one later pass fails after helper progress moves forward, and prove the stored state clearly shows helper progress ahead of real-table progress
- [x] GREEN: ensure full-pass success updates every mapped table together while failure leaves previous per-table success frontiers intact and records the specific failure
- [x] REFACTOR: make the "tables whose sync state should advance or clear errors" input explicit so tracking persistence does not infer it from raw runtime internals

### Slice 4: Restart And Repeatable Resume

- [x] RED: add failing coverage that restarts the runner after both a success and a failure and proves bootstrap reruns preserve reconciled watermarks, per-table success state, and prior errors while older replay does not regress tracked success
- [x] GREEN: keep bootstrap merge-only and preserve monotonic reconcile/frontier semantics across restart
- [x] REFACTOR: deduplicate watermark-merge rules if resolved ingest and reconcile success currently compare or render the same frontier data in multiple places

### Slice 5: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm tracking ownership no longer leaks through the webhook module and that no duplicate state shapes were introduced

## Boundary Review Checklist

- [x] No reconcile or bootstrap code depends on a webhook-owned tracking module anymore
- [x] No reconcile failure is swallowed, downgraded, or only logged without durable tracking-state evidence
- [x] No failed pass advances `latest_reconciled_resolved_watermark` or `last_successful_sync_watermark`
- [x] No bootstrap rerun wipes `last_error`, reconciled watermark, or prior successful sync progress
- [x] No new cutover-ready flag or history table is invented just to answer the acceptance criteria
- [x] No duplicate mapping/runtime metadata shape is introduced only for tracking persistence

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
