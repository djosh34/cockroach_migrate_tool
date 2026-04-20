## Task: Make the default-branch publish workflow succeed from a plain `git push` <status>not_started</status> <passes>true</passes>

<blocked_by>.ralph/tasks/story-21-github-workflows-image-publish/02b-task-drive-three-image-github-pipeline-under-fifteen-minutes-with-native-arm64-and-workflow-restructure.md</blocked_by>

<description>
Must use tdd skill to complete


**Goal:** The hosted publish workflow is still failing in `.github/workflows/publish-images.yml`, which means the default-branch push path does not yet work the way the product needs it to. This task exists to make the repository owner experience dead simple: a plain `git push` to the default branch must be enough to trigger the full hosted publish workflow successfully, without manual reruns, manual dispatches, or extra recovery steps. The higher order goal is to make the supported release path trustworthy and routine instead of fragile and operator-dependent.

In scope:
- debug the current hosted failure in `.github/workflows/publish-images.yml`
- ensure the intended push-triggered publish path works from a normal `git push` to the repository default branch
- verify that no manual workflow-dispatch step, ad hoc rerun button, follow-up empty commit, or other babysitting step is required to get the publish path to complete
- preserve the intended branch-gating model so trusted publish behavior remains limited to the default branch push path
- use authenticated hosted workflow inspection to confirm the real GitHub Actions run succeeds rather than trusting local guesses
- fix workflow trigger wiring, job conditions, dependencies, permissions, concurrency, or artifact handoff issues as needed so the push path actually completes
- target the repository’s real default branch, `master`, for the trusted push-triggered publish path

Out of scope:
- introducing manual-only publication as a substitute for a broken push path
- weakening security or correctness gates just to make a push-triggered run go green
- treating a locally simulated run as sufficient evidence

Decisions already made:
- the failing `.github/workflows/publish-images.yml` run is evidence that the default-branch push path is still broken
- the supported operator flow should be “push to the default branch and let the hosted workflow do its job”
- the trusted publish branch for this repository is `master`
- manual reruns or manual dispatches are not an acceptable substitute for a working push-triggered publish workflow
- this work should rely on real hosted GitHub workflow evidence before being considered done

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers the intended push-triggered publish workflow contract where practical
- [ ] Real hosted GitHub Actions evidence shows `.github/workflows/publish-images.yml` succeeds from a normal `git push` to the default branch
- [ ] No manual workflow dispatch, manual rerun, empty follow-up commit, or comparable babysitting step is required for the supported push-triggered publish flow
- [ ] The workflow keeps the intended `master`-only trusted publish boundary and does not accidentally broaden secret or publish access to other event types or branches
- [ ] The task reflects the actual hosted failure modes fixed while making the push path succeed
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
