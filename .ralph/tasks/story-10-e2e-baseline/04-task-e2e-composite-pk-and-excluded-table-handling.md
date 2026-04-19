## Task: End-to-end test composite primary keys and excluded tables <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test that proves the migration supports composite primary keys and table exclusion rules. The higher order goal is to validate that the schema and routing machinery is usable on more realistic schemas than simple integer-id tables.

In scope:
- composite PK tables
- excluded table handling
- helper shadow persistence
- continuous reconcile
- MOLT verify for the included real tables

Out of scope:
- source load chaos

</description>


<acceptance_criteria>
- [x] Red/green TDD covers a real migration containing composite PK tables and excluded tables
- [x] Included tables migrate correctly and excluded tables are skipped intentionally
- [x] MOLT verify or equivalent real-table verification passes for the intended included set
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-10-e2e-baseline/04-task-e2e-composite-pk-and-excluded-table-handling_plans/2026-04-19-composite-pk-excluded-table-e2e-plan.md</plan>
