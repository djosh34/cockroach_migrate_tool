## Task: Fix GitHub workflows to build, test, and publish the three-image split in the right order <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a dedicated workflow story that updates GitHub Actions for the new three-image architecture before novice-user verification depends on published artifacts, and keep iterating against real workflow failures until image builds actually work. The higher order goal is to ensure registry-published images are the real supported product path and that downstream verification uses exactly what CI publishes rather than wishful local assumptions.

In scope:
- workflows for building, testing, and publishing the verify image
- workflows for building, testing, and publishing the one-time SQL-emitter image
- workflows for building, testing, and publishing the runner scratch image
- ordering and dependency rules so downstream verification uses the published images from the workflow outputs
- checks that fail if the registry-only novice-user path is attempted before image publication is wired correctly
- manual installation of workflow dependencies instead of relying on potentially insecure third-party GitHub Actions where a direct install step is practical
- explicit restriction against casually importing external community actions just because they are convenient
- building and publishing both `arm64` and `amd64` image variants
- using authenticated GitHub workflow/API log inspection to read real image-building logs and results until the workflows work for real
- trigger policy that runs the publish workflow only on pushes to `master`
- publishing images tagged by the exact full pushed commit SHA rather than a floating `latest` tag
- concurrency control that cancels the previous in-progress run when a newer push lands on `master`
- explicit non-support for publishing from pull requests, forked contributions, or other external-person PR paths
- manual release tagging flow only, with release creation intentionally restricted to repository owners
- ordering that ensures images are never pushed before their required tests pass

Out of scope:
- implementing the image internals themselves
- novice-user verification scenarios beyond what is needed to publish the artifacts

Decisions already made:
- this must be a story on its own
- it must happen before the novice-user registry-only verification story
- published images, not local builds, are the novice-user contract
- image builds do not work yet and this story must reflect real CI/debugging work rather than paper design
- workflow dependencies should be installed manually where possible rather than pulled in via random external actions
- the trust bar for external actions is extremely high; prefer direct shell installation over importing extra actions
- both arm and x86 images are required
- use the authenticated GitHub API curl wrapper/skill to inspect workflow runs and build logs until the build is actually fixed
- publish happens on commits to `master`, not on PRs
- image tags for this automated flow should use the full commit SHA and should not publish `latest`
- previous in-progress publish runs should be cancelled when a newer `master` push arrives
- releases are always manual and only repository owners should be able to cut them
- pushing before tests pass is explicitly disallowed

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers workflow behavior for build, test, and publish across the three-image split
- [ ] GitHub workflows publish the verify, SQL-emitter, and runner images in a form the novice-user flow can consume directly from the registry
- [ ] The workflows build and publish both `arm64` and `amd64` images
- [ ] Workflow definitions manually install required dependencies where practical and avoid importing untrusted third-party actions
- [ ] The task is not complete until authenticated GitHub workflow log inspection has been used to confirm the real image-building runs succeed
- [ ] Automated image publish runs trigger only on pushes to `master`, not on pull requests or external contribution paths
- [ ] Published image tags use the exact full commit SHA and do not rely on a floating `latest` tag for this workflow
- [ ] Workflow concurrency cancels the previous in-progress `master` publish run when a newer push arrives
- [ ] No image is pushed before its required tests pass successfully
- [ ] Release tagging remains a manual owner-controlled path and is not automatically performed by the publish workflow
- [ ] Downstream checks fail if the registry-only user path is not backed by published images from CI
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
