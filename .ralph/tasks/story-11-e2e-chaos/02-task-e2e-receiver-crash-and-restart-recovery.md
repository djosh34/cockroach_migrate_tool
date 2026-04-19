## Task: End-to-end test receiver crash and restart recovery <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test where the destination runner crashes after helper-state persistence or during reconcile and then recovers correctly. The higher order goal is to prove the chosen helper-shadow architecture is restartable under realistic failure.

In scope:
- runner crash after helper persistence
- runner crash during reconcile
- restart behavior
- eventual correctness of real tables

Out of scope:
- source-side manual rescue commands

</description>


<acceptance_criteria>
- [x] Red/green TDD covers crash-and-restart recovery end to end
- [x] No extra source-side commands are used to rescue the migration after CDC setup
- [x] Real target tables converge correctly after restart
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-11-e2e-chaos/02-task-e2e-receiver-crash-and-restart-recovery_plans/2026-04-19-receiver-crash-restart-recovery-plan.md</plan>
