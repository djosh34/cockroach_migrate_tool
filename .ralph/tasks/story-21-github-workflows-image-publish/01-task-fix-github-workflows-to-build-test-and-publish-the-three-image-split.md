## Task: Fix GitHub workflows to build, test, and publish the three-image split in the right order <status>completed</status> <passes>true</passes>

<blocked_by>.ralph/tasks/story-21-github-workflows-image-publish/02b-task-drive-three-image-github-pipeline-under-fifteen-minutes-with-native-arm64-and-workflow-restructure.md</blocked_by>

<description>
Must use tdd skill to complete


**Goal:** Create a dedicated workflow story that updates GitHub Actions for the new three-image architecture before novice-user verification depends on published artifacts, and keep iterating against real workflow failures until image builds actually work. This task is suspended until the under-fifteen-minute pipeline speed gate in the same story passes, so workflow-fix work happens on top of a workflow that is actually fast enough to be acceptable. The higher order goal is to ensure registry-published images are the real supported product path and that downstream verification uses exactly what CI publishes rather than wishful local assumptions.

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
- trigger policy that runs the publish workflow only on pushes to `main`
- publishing images tagged by the exact full pushed commit SHA rather than a floating `latest` tag
- concurrency control that cancels the previous in-progress run when a newer push lands on `main`
- explicit non-support for publishing from pull requests, forked contributions, or other external-person PR paths
- manual release tagging flow only, with release creation intentionally restricted to repository owners
- ordering that ensures images are never pushed before their required tests pass
- secrets handling rules that ensure registry credentials are available only to the intended trusted `main`-push publish path
- explicit masking/redaction steps for any sensitive value that is not automatically handled as a GitHub secret
- verification that logs do not leak registry credentials and that GitHub masking/redaction is working correctly
- explicit protection against issue-triggered, PR-triggered, branch-triggered, or other unintended workflow paths receiving publish secrets

Out of scope:
- implementing the image internals themselves
- novice-user verification scenarios beyond what is needed to publish the artifacts

Decisions already made:
- this must be a story on its own
- this task is suspended until `.ralph/tasks/story-21-github-workflows-image-publish/02b-task-drive-three-image-github-pipeline-under-fifteen-minutes-with-native-arm64-and-workflow-restructure.md` passes
- it must happen before the novice-user registry-only verification story
- published images, not local builds, are the novice-user contract
- image builds do not work yet and this story must reflect real CI/debugging work rather than paper design
- workflow dependencies should be installed manually where possible rather than pulled in via random external actions
- the trust bar for external actions is extremely high; prefer direct shell installation over importing extra actions
- both arm and x86 images are required
- use the authenticated GitHub API curl wrapper/skill to inspect workflow runs and build logs until the build is actually fixed
- publish happens on commits to `main`, not on PRs
- image tags for this automated flow should use the full commit SHA and should not publish `latest`
- previous in-progress publish runs should be cancelled when a newer `main` push arrives
- releases are always manual and only repository owners should be able to cut them
- pushing before tests pass is explicitly disallowed
- publish secrets must not be usable from PRs, issues, other branches, forks, or other unintended event types
- secret redaction behavior must be treated as a first-class security requirement rather than assumed to work magically

</description>


<acceptance_criteria>
- [x] Red/green TDD covers workflow behavior for build, test, and publish across the three-image split
- [x] GitHub workflows publish the verify, SQL-emitter, and runner images in a form the novice-user flow can consume directly from the registry
- [x] The workflows build and publish both `arm64` and `amd64` images
- [x] Workflow definitions manually install required dependencies where practical and avoid importing untrusted third-party actions
- [x] The task is not complete until authenticated GitHub workflow log inspection has been used to confirm the real image-building runs succeed
- [x] Automated image publish runs trigger only on pushes to `main`, not on pull requests, issues, other branches, or external contribution paths
- [x] Published image tags use the exact full commit SHA and do not rely on a floating `latest` tag for this workflow
- [x] Workflow concurrency cancels the previous in-progress `main` publish run when a newer push arrives
- [x] No image is pushed before its required tests pass successfully
- [x] Release tagging remains a manual owner-controlled path and is not automatically performed by the publish workflow
- [x] Workflow design proves publish secrets are unavailable to untrusted events and verifies masking/redaction works correctly in logs
- [x] Downstream checks fail if the registry-only user path is not backed by published images from CI
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<outcome>
- Verified the final split workflow locally through the repo-owned contract boundary and the required local gates: `make check`, `make lint`, and `make test` all passed on `main` at `5fcfee60d11ca39e52830c5b6d9707114882b5f0`.
- Authenticated hosted inspection used `/home/joshazimullah.linux/github-api-curl` against `https://github.com/djosh34/cockroach_migrate_tool/actions/runs/24643444655`, which completed successfully from `2026-04-20T00:53:18Z` to `2026-04-20T01:10:07Z`.
- The hosted run proved the intended ordering: `validate-fast` succeeded, `validate-long` succeeded, only then did the six `publish-image` matrix jobs start, and `publish-manifest` completed successfully afterward.
- Hosted log inspection confirmed the redaction path is real, not assumed: every publish lane and `publish-manifest` logged `derived registry auth (masked): ***`, and the native-runner assertion logs showed both `runner.arch=X64` and `runner.arch=ARM64` in the expected platform lanes.
- The workflow now truthfully publishes the runner, setup-sql, and verify images through the registry-first contract, and the shared workflow/published-image support boundaries remained the cleanest code boundary after the final review.
</outcome>

<plan>.ralph/tasks/story-21-github-workflows-image-publish/01-task-fix-github-workflows-to-build-test-and-publish-the-three-image-split_plans/2026-04-19-three-image-main-publish-workflow-plan.md</plan>
