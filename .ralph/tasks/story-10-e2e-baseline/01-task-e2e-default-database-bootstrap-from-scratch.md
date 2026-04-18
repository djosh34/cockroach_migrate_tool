## Task: End-to-end test a default database bootstrap from scratch <status>not_started</status> <passes>false</passes>

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
- [ ] Red/green TDD covers a real end-to-end default-environment migration from setup through verification
- [ ] The test proves required Cockroach changes are explicit and destination helper bootstrap is automatic
- [ ] No extra source-side commands are used after CDC setup completes
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

