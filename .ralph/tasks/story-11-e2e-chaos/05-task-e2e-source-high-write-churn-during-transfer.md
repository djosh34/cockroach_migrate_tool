## Task: End-to-end test high source write churn during transfer <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test with high source write churn during migration, including repeated create/delete activity in source data patterns, and evaluate how the system behaves. The higher order goal is to discover whether the selected design remains stable and whether targeted improvements are needed.

In scope:
- heavy source write activity during shadowing
- helper persistence stability
- continuous reconcile behavior under load
- final real-table correctness

Out of scope:
- changing the selected design unless the test proves a concrete need

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers high source write churn during migration end to end
- [ ] The test is imposed from outside the runner, not by hidden test logic in the binary
- [ ] The task records whether the current design handles churn acceptably or exposes a concrete improvement need
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

