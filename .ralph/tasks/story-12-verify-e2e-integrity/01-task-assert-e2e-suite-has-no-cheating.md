## Task: Assert the end-to-end suite has no cheating <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Build explicit checks that the end-to-end suite is truly end to end and does not hide fake migrations, shortcuts, magic side channels, or test-only control paths. The higher order goal is to make the test suite itself trustworthy.

In scope:
- assert no fake migration shortcuts
- assert no hidden "cheat" toggles in tests
- assert the real webhook and reconcile path is used
- assert MOLT verify checks the real destination tables

Out of scope:
- adding new migration features

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers assertions that the E2E suite uses the real migration path only
- [ ] The suite rejects fake migrations, shortcuts, and extra magic
- [ ] The integrity checks are part of the repository and run automatically
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

