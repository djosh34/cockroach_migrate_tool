## Task: End-to-end test transaction-failure recovery <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test for transaction failures on the destination side and prove the system recovers without silent loss or fake recovery shortcuts. The higher order goal is to validate the transactional contract around helper persistence and reconcile.

In scope:
- failed helper persistence transaction
- failed reconcile transaction
- retry and recovery behavior
- eventual correctness of the real target tables

Out of scope:
- manual post-failure shell repair

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers destination transaction failures and recovery end to end
- [ ] Failure does not lead to silent data loss or skipped work
- [ ] Real target tables converge correctly after recovery
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

