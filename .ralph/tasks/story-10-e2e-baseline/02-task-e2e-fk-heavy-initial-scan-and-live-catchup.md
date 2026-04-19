## Task: End-to-end test FK-heavy initial scan and live catch-up <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test for FK-heavy schemas that proves the selected helper-shadow plus reconcile design handles initial scan and live catch-up while the real tables keep constraints enabled. The higher order goal is to validate the main reason this design was chosen over direct apply.

In scope:
- parent/child/grandchild schema
- initial scan
- live updates during shadowing
- repeated reconcile
- MOLT verify on real tables

Out of scope:
- external chaos injection

</description>


<acceptance_criteria>
- [x] Red/green TDD covers a real FK-heavy migration from initial scan through live catch-up
- [x] Real target tables keep PK/FK constraints enabled throughout the test
- [x] MOLT verify confirms the real target tables match source state
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-10-e2e-baseline/02-task-e2e-fk-heavy-initial-scan-and-live-catchup_plans/2026-04-19-fk-heavy-e2e-plan.md</plan>
