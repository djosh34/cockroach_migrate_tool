## Task: Add a manual image-retag workflow that promotes already-published commit images to a requested version and optional `latest` <status>suspended</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Create a separate manual GitHub Actions workflow that lets a trusted operator retag the already-published commit-SHA images in the registry as a requested version tag and, optionally, also as `latest`, without rebuilding images or rerunning the full repository test suite. This task is suspended because this is out of scope for the story right now; first continue with other stories. The higher order goal is to separate slow correctness validation from deliberate release-tag promotion so a known-good published commit can be promoted to human-friendly tags safely, quickly, and repeatably.

In scope:
- a new manual workflow dedicated to retagging/promoting existing published images rather than rebuilding them
- `workflow_dispatch` style manual usage through the GitHub Actions UI with a required version input and a boolean/checkbox-style "set as latest" input
- owner/trusted-operator-only use expectations for that manual workflow
- pulling the already-published images by commit-SHA tag and pushing them back under the requested version tag, or using direct registry retagging if the target registry supports it cleanly and safely
- applying the manual retag flow to the full image set in this story rather than only one image
- ensuring the workflow uses the latest committed/pushed published images as its source of truth rather than rebuilding locally
- explicit non-goal of rerunning the entire validation/test matrix during the retag step
- contract coverage and workflow tests that prove this workflow is manual-only and release-tag oriented
- verification of the manual workflow through API-based invocation, not just local YAML inspection

Out of scope:
- rebuilding images from source during retag/promotion
- rerunning `make check`, `make lint`, `make test`, or `make test-long` as part of the retag workflow itself
- replacing the main publish workflow that creates the original commit-SHA images
- automatic release promotion on every push

Decisions already made:
- this task must be the last task in the GitHub workflows image-publish story
- this task is suspended because this is out of scope for the story right now; first continue with other stories
- the workflow is manual, not push-triggered
- a user/operator must set the version parameter and optionally tick a "set as latest" box when dispatching it
- this path should promote already-published images instead of redoing the entire test/build pipeline
- pulling and re-pushing existing images is acceptable, and direct registry retagging is preferred if it is actually supported and keeps the workflow simpler/safer
- the task is not complete without real verification of manual invocation through the GitHub API/workflow-dispatch path
- this workflow should remain clearly distinct from the trusted main publish workflow so release promotion is an explicit action

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers the manual retag workflow contract and proves it is separate from the main publish workflow
- [ ] A new manual workflow accepts a required version input and a boolean/checkbox-style "set as latest" input through GitHub Actions `workflow_dispatch`
- [ ] The manual workflow promotes the already-published commit-SHA image set to the requested version tag without rebuilding images or rerunning the full validation/test suite
- [ ] When the operator selects the "set as latest" option, the workflow also applies the `latest` tag; when not selected, it does not
- [ ] The workflow operates on the full published image set for this story and uses already-published registry artifacts as the source of truth
- [ ] Real verification includes successful manual invocation through the GitHub API/workflow-dispatch path, not just YAML contract tests
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
