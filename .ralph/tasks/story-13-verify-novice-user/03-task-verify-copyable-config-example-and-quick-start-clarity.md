## Task: Verify the copyable config example and quick start are directly useful <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create an explicit verification task that proves the quick-start documentation contains a copyable starting config example and minimal steps that work as written. The higher order goal is to tune the operator experience for a novice who will not investigate anything outside the README.

In scope:
- copyable config example
- quick-start steps
- clarity and minimalism requirements
- failure if the user must infer undocumented behavior

Out of scope:
- full reference documentation

</description>


<acceptance_criteria>
- [x] Red/green TDD covers a real quick-start path using the documented sample config and steps
- [x] The task fails if the user must infer undocumented steps or look up extra behavior elsewhere
- [x] The README quick start is directly useful, concise, and copyable
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-13-verify-novice-user/03-task-verify-copyable-config-example-and-quick-start-clarity_plans/2026-04-19-copyable-config-quick-start-plan.md</plan>
