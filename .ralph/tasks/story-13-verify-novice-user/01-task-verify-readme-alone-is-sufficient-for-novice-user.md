## Task: Verify the README alone is sufficient for a novice user <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create an explicit verification task that proves a novice user can complete the migration setup path from the README alone without reading code or inspecting arbitrary repo files. The higher order goal is to make the quick-start path truly usable rather than merely documented.

In scope:
- README-only operator path
- no source-code reading requirement
- no “look up how this works” requirement
- direct, minimal steps

Out of scope:
- deep architecture documentation

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers a README-only user path verification
- [ ] The task fails if the user must inspect source code or repo internals to complete the path
- [ ] The quick start is short, direct, and sufficient on its own
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

