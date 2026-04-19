## Task: Build a scratch verify image from the pruned verify-only source <status>not_started</status> <passes>false</passes>

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
- [ ] Red/green TDD covers building and running the verify image from the verify-only source slice
- [ ] The produced verify image is scratch-based and contains only what is needed to run the verify service
- [ ] Tests fail if unrelated runtime content or extra source behavior leaks into the image
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
