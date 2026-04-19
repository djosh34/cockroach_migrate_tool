## Task: End-to-end test high source write churn during transfer <status>done</status> <passes>true</passes>

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

Verdict:
- The current design handled this bounded customer-write churn workload acceptably.
- No concrete design change was required; the new long-lane test converged through helper shadow state, real destination state, tracking catch-up, runner liveness, and `runner verify`.

</description>


<acceptance_criteria>
- [x] Red/green TDD covers high source write churn during migration end to end
- [x] The test is imposed from outside the runner, not by hidden test logic in the binary
- [x] The task records that the current design handled bounded churn acceptably and did not expose a concrete improvement need
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-11-e2e-chaos/05-task-e2e-source-high-write-churn-during-transfer_plans/2026-04-19-source-write-churn-e2e-plan.md</plan>
