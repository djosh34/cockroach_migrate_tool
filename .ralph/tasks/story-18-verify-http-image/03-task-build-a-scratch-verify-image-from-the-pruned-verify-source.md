## Task: Build a scratch verify image from the pruned verify-only source <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Produce a dedicated scratch-based verify image that is built only from the pruned verify source slice. The higher order goal is to make the verify runtime minimal, isolated, and publishable as its own artifact.

In scope:
- Dockerfile and build pipeline for the verify image
- scratch-container packaging
- binary/image contract tests
- published-image assumptions needed by later novice-user verification

Out of scope:
- implementing verify job HTTP endpoints

Decisions already made:
- the verify image should be a scratch container as well
- it must not drag along unrelated source or runtime behavior

</description>


<acceptance_criteria>
- [x] Red/green TDD covers building and running the verify image from the verify-only source slice
- [x] The produced verify image is scratch-based and contains only what is needed to run the verify service
- [x] Tests fail if unrelated runtime content or extra source behavior leaks into the image
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-18-verify-http-image/03-task-build-a-scratch-verify-image-from-the-pruned-verify-source_plans/2026-04-19-verify-scratch-image-plan.md</plan>
