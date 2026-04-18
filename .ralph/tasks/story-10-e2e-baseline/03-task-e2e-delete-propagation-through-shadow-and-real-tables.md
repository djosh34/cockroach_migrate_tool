## Task: End-to-end test delete propagation through helper shadow and real tables <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test that proves deletes land in helper shadow tables and are then removed from the real tables by the continuous SQL-driven delete reconcile pass. The higher order goal is to validate the chosen simple delete model.

In scope:
- source deletes
- helper shadow delete state
- periodic SQL delete refresh into the real tables
- MOLT verify on real tables

Out of scope:
- high-load chaos

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers real delete propagation from source through shadow tables into real tables
- [ ] The real target tables lose rows through periodic SQL refresh based on shadow-table absence
- [ ] MOLT verify confirms delete correctness on the real tables
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

