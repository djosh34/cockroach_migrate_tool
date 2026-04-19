## Task: End-to-end test HTTP retry chaos imposed externally <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a real end-to-end test where webhook HTTP failures are imposed from outside the runner and Cockroach retries until helper persistence succeeds. The higher order goal is to prove retry correctness without cheating by embedding test-only shortcut logic in the binary.

In scope:
- external failure injection on HTTP responses
- duplicate delivery handling
- helper shadow idempotency
- eventual converge to correct real-table state

Out of scope:
- fake test-only webhook shortcuts inside the binary

</description>


<acceptance_criteria>
- [x] Red/green TDD covers externally imposed HTTP failure and retry behavior end to end
- [x] The binary contains no test-only shortcut logic for this scenario
- [x] Real target tables converge correctly after retries
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-11-e2e-chaos/01-task-e2e-http-retry-chaos-imposed-externally_plans/2026-04-19-http-retry-chaos-e2e-plan.md</plan>
