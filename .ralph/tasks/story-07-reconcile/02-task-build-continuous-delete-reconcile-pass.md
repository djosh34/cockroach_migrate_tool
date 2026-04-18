## Task: Build the continuous SQL-driven delete reconcile pass <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Build the simple SQL-driven delete path where PostgreSQL removes rows from the real tables when they no longer exist in the helper shadow tables. The higher order goal is to implement the selected delete model without adding tombstone machinery unless performance later proves it necessary.

In scope:
- reverse-dependency delete passes
- anti-join or equivalent SQL delete shape
- repeatable execution
- sync-state updates after success

Out of scope:
- tombstone optimization
- cutover logic

This task must preserve the explicit decision:
- deletes are propagated by SQL during periodic refresh from helper shadow tables into the real tables

</description>


<acceptance_criteria>
- [x] Red/green TDD covers delete propagation from helper shadow absence into real tables
- [x] Delete reconcile runs in child-before-parent order
- [x] Repeated delete passes remain safe and idempotent
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-07-reconcile/02-task-build-continuous-delete-reconcile-pass_plans/2026-04-18-continuous-delete-reconcile-plan.md</plan>
