## Task: Build the continuous SQL-driven delete reconcile pass <status>not_started</status> <passes>false</passes>

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
- [ ] Red/green TDD covers delete propagation from helper shadow absence into real tables
- [ ] Delete reconcile runs in child-before-parent order
- [ ] Repeated delete passes remain safe and idempotent
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

