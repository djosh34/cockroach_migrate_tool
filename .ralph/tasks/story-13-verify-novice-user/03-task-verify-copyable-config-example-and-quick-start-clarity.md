## Task: Verify the copyable config example and quick start are directly useful <status>not_started</status> <passes>false</passes>

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
- [ ] Red/green TDD covers a real quick-start path using the documented sample config and steps
- [ ] The task fails if the user must infer undocumented steps or look up extra behavior elsewhere
- [ ] The README quick start is directly useful, concise, and copyable
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
