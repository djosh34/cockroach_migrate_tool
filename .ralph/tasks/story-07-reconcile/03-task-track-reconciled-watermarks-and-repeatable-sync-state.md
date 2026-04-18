## Task: Track reconciled watermarks and repeatable sync state <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Track the state that proves how far the continuous reconcile loop has progressed. The higher order goal is to make drain-to-zero, restart, and cutover readiness visible without overengineering the state model.

In scope:
- latest reconciled resolved watermark
- per-table last successful sync
- reconcile error tracking
- repeatable resume behavior

Out of scope:
- source bootstrap
- MOLT verification wrapper

</description>


<acceptance_criteria>
- [x] Red/green TDD covers reconciled-watermark updates, repeatability, and restart behavior
- [x] Sync state is sufficient to tell whether real tables have caught up to helper-state progress
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-07-reconcile/03-task-track-reconciled-watermarks-and-repeatable-sync-state_plans/2026-04-18-reconciled-watermarks-and-repeatable-sync-state-plan.md</plan>
