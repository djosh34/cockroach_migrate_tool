## Task: End-to-end test a default database bootstrap from scratch <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test that starts from a default CockroachDB and PostgreSQL setup and proves the migration can be bootstrapped end to end without hidden manual intervention. The higher order goal is to verify the basic operator path on a naked/default environment.

In scope:
- default source bootstrap
- automatic destination helper bootstrap
- HTTPS webhook path
- helper shadow persistence
- continuous reconcile into real tables
- MOLT verify against real tables

Out of scope:
- chaos/fault injection

This test must not rely on extra source-side commands after CDC setup is completed.

</description>


<acceptance_criteria>
- [x] Red/green TDD covers a real end-to-end default-environment migration from setup through verification
- [x] The test proves required Cockroach changes are explicit and destination helper bootstrap is automatic
- [x] No extra source-side commands are used after CDC setup completes
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-10-e2e-baseline/01-task-e2e-default-database-bootstrap-from-scratch_plans/2026-04-19-default-bootstrap-e2e-plan.md</plan>
