## Task: Assert there are no post-setup source commands in end-to-end tests <status>completed</status> <passes>true</passes>

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
- [x] Red/green TDD covers detection of forbidden post-setup source-side commands in E2E tests
- [x] The E2E suite fails if it relies on extra source-side intervention after CDC setup
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-12-verify-e2e-integrity/03-task-assert-no-post-setup-source-commands-in-e2e_plans/2026-04-19-no-post-setup-source-commands-plan.md</plan>
