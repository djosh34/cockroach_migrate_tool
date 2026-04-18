## Task: End-to-end test HTTP retry chaos imposed externally <status>not_started</status> <passes>false</passes>

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
- [ ] Red/green TDD covers externally imposed HTTP failure and retry behavior end to end
- [ ] The binary contains no test-only shortcut logic for this scenario
- [ ] Real target tables converge correctly after retries
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

