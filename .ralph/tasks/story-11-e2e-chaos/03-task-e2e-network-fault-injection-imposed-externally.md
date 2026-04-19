## Task: End-to-end test externally imposed network fault injection <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test where network faults are imposed from outside the runner, not via hidden test hooks inside the binary. The higher order goal is to prove the system handles transport instability honestly.

In scope:
- external network fault injection
- retry/resume behavior
- continuous reconcile catch-up after recovery

Out of scope:
- binary-level test-only network stubs

</description>


<acceptance_criteria>
- [x] Red/green TDD covers externally imposed network instability end to end
- [x] No hidden in-binary test shortcut is used for network chaos
- [x] Real target tables converge after the network recovers
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-11-e2e-chaos/03-task-e2e-network-fault-injection-imposed-externally_plans/2026-04-19-external-network-fault-e2e-plan.md</plan>
