## Task: Assert there are no post-setup source commands in end-to-end tests <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Build explicit checks that, once CDC setup is done, the end-to-end suite does not keep issuing extra commands against the source database to make the migration work. The higher order goal is to match the real operational constraint that the destination side should carry the migration after source setup.

In scope:
- assert no extra source shell commands after setup
- assert no hidden helper SQL against the source after setup
- assert migration progress depends on the destination container, not more source-side intervention

Out of scope:
- general chaos testing

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers detection of forbidden post-setup source-side commands in E2E tests
- [ ] The E2E suite fails if it relies on extra source-side intervention after CDC setup
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

