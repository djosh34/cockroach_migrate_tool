## Task: End-to-end test multiple large multi-database migrations under one container <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test that drives multiple large and complicated migrations from multiple source databases to multiple destination databases through one destination container. The higher order goal is to validate the actual runtime shape required in production rather than a toy single-database scenario.

In scope:
- multiple source databases
- multiple destination databases
- one destination container
- one webhook endpoint runtime
- helper shadow persistence and continuous reconcile for all mappings
- MOLT verify on the real target tables

Out of scope:
- external fault injection

</description>


<acceptance_criteria>
- [x] Red/green TDD covers multiple large multi-db migrations controlled by one destination container
- [x] The test proves one container can own all configured mappings without cross-talk
- [x] Real target tables verify correctly after migration
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-10-e2e-baseline/05-task-e2e-multiple-large-multi-db-migrations_plans/2026-04-19-multi-mapping-e2e-plan.md</plan>
