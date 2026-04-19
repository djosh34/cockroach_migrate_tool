## Task: Build the runner as a scratch image with one binary that only applies webhook requests to PostgreSQL <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Produce the final runtime runner image as a scratch container containing one binary whose job is to apply incoming webhook requests onto the real PostgreSQL database. The higher order goal is to keep the runtime image extremely small, tightly scoped, and free of bootstrap or verification responsibilities.

In scope:
- scratch-container runner image
- single-binary runtime contract
- webhook ingestion and application to PostgreSQL
- image/runtime tests that prove the container does only the runner job

Out of scope:
- verify behavior
- source SQL generation

Decisions already made:
- the runner image should be scratch with one binary
- the runner only applies webhook requests onto the real PostgreSQL database
- the runner image is separate from verify and setup images

</description>


<acceptance_criteria>
- [x] Red/green TDD covers the single-binary scratch runner image and its PostgreSQL apply path
- [x] The runner image contains only the runner binary and the minimal runtime contents needed to operate
- [x] The runner performs webhook application to PostgreSQL and nothing else
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-20-runner-scratch-image/01-task-build-the-runner-as-a-scratch-image-with-one-binary-that-applies-webhook-requests-to-postgresql_plans/2026-04-19-runner-scratch-image-apply-plan.md</plan>
