## Task: Upgrade the verify-only slice to Go 1.26 and bump its dependencies before packaging the image <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Upgrade the extracted verify slice to Go 1.26 and refresh its dependencies before the image contract is locked in. The higher order goal is to avoid freezing a brand-new image on stale toolchain and dependency choices.

In scope:
- migrate the verify slice to Go 1.26
- bump dependencies used by the verify slice
- resolve test, lint, and compatibility fallout within the verify slice

Out of scope:
- upgrading unrelated code that is no longer part of the verify slice

Decisions already made:
- the verify path should be modernized during the split
- the dependency and toolchain refresh belongs in the verify-image workstream rather than later cleanup

</description>


<acceptance_criteria>
- [x] Red/green TDD covers the Go 1.26 and dependency upgrade path for the verify slice
- [x] The verify slice builds, tests, and lints cleanly on Go 1.26
- [x] Dependency updates are constrained to the verify slice and do not preserve dead legacy code
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-18-verify-http-image/02-task-upgrade-the-verify-slice-to-go-1-26-and-bump-all-dependencies_plans/2026-04-19-verify-go-1-26-dependency-upgrade-plan.md</plan>
