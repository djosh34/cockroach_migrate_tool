## Task: Require `make test-long` to pass before any image publish or release path can proceed <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Make the ultra-long test lane a hard prerequisite for image publication so GitHub Actions cannot publish the runner, verify, or SQL-emitter images until `make test-long` has completed successfully. The higher order goal is to eliminate false confidence from fast-only lanes and ensure the published registry artifacts are backed by the full required repository test surface, including the long-running end-to-end contracts.

In scope:
- workflow changes that make `make test-long` an explicit required dependency before any publish step can run
- ensuring the publish graph waits for `make check`, `make lint`, `make test`, and `make test-long`
- contract coverage that fails loudly if a later workflow edit weakens or bypasses the `make test-long` requirement
- preserving trusted-trigger and post-test publish ordering while adding the long-lane gate
- making the required lane clear in workflow outputs, job dependencies, and repository contracts so reviewers can see that publish is blocked on the full suite
- ensuring the gate applies to the whole image publish flow rather than only one of the three images
- ensuring future release-oriented image publication paths inside this story do not bypass the `make test-long` requirement

Out of scope:
- reducing the scope of `make test-long` to make publication easier
- silently downgrading failing long-lane cases to warnings or non-blocking signals
- unrelated speed optimizations except where needed to make the new gate practical

Decisions already made:
- this is a separate task in the GitHub image workflow story
- `make test-long` must be a requirement to publish
- publishing with only the short/default test lanes is not acceptable
- the publish workflow must fail loudly if the long lane fails or is skipped
- the gate should cover the real supported image publication path, not an optional side lane
- this task belongs before the later compose/debug/quay follow-up tasks in story 21

</description>

<outcome>
- Replaced the mixed `validate` pre-publish boundary in `.github/workflows/publish-images.yml` with explicit `validate-fast` and `validate-long` jobs, and made `publish-image` wait on both before any image publication can start.
- Made the fast boundary visibly run `make check`, `make lint`, and `make test`, and made the long boundary visibly run `make test-long`, so the publish graph now shows the full required suite instead of hiding the long lane behind implicit shell behavior.
- Refactored `GithubWorkflowContract` so validation topology, permission isolation, cache reuse, install steps, and README safety documentation all have one honest owner for the new fast/long gate.
- Updated the CI publish safety documentation to describe the explicit fast/long validation gate without leaking contributor-only validation command guidance into the operator quick-start path.
- Fixed two unrelated but real test-boundary issues discovered while proving the required gates: a Postgres test-harness port-ownership race in the default suite and stale long-lane harness assumptions around fully reconciled tracking state and multi-mapping bootstrap command counts.
</outcome>


<acceptance_criteria>
- [x] Red/green TDD covers the workflow contract that blocks all image publication until `make test-long` passes
- [x] GitHub Actions requires `make check`, `make lint`, `make test`, and `make test-long` to succeed before any runner, verify, or SQL-emitter image is published
- [x] Workflow contract tests fail loudly if later edits skip, weaken, or bypass the `make test-long` gate
- [x] The long-lane requirement is visible in the workflow dependency graph rather than hidden behind implicit shell behavior
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-21-github-workflows-image-publish/03-task-require-make-test-long-to-pass-before-any-image-publish_plans/2026-04-20-test-long-publish-gate-plan.md</plan>
