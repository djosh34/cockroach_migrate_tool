## Task: Debug real GitHub image-build failures using authenticated workflow API log access until the published runs succeed <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Add an explicit task for debugging failing GitHub Actions image builds against the real hosted workflow runs instead of reasoning from local guesses alone. The higher order goal is to make the workflow story evidence-based: the image pipeline is only fixed when the hosted GitHub runs and logs show it is fixed.

In scope:
- inspect workflow runs, jobs, and logs for failing image builds through authenticated GitHub API access
- use the local authenticated GitHub API curl wrapper/skill instead of exposing tokens or relying on unauthenticated guesses
- iterate on workflow/task fixes until the hosted image-build runs succeed for the three-image split
- capture the real causes of failure found in hosted CI, including architecture-specific failures
- inspect real hosted logs for secret-masking/redaction behavior as part of workflow verification
- verify that trusted-secret usage is gated to the intended `main` push path only

Out of scope:
- broad repository triage unrelated to image build and publish workflows
- hiding or swallowing workflow failures

Decisions already made:
- image builds do not work at all right now
- the fix must be validated against real GitHub workflow logs/results
- the authenticated GitHub API curl wrapper/skill is the intended path for inspecting workflow runs safely
- both `arm64` and `amd64` image paths matter during this debugging work
- real log inspection should include checking that redaction is functioning correctly
- secret-gating failures are workflow bugs and must be treated as real failures

</description>


<acceptance_criteria>
- [x] Red/green TDD covers the local logic around workflow/result expectations where practical
- [x] Real hosted GitHub workflow runs and logs have been inspected through authenticated API access until the image builds succeed
- [x] The task records or reflects the actual CI failure modes fixed, including any arch-specific publish failures and any secret-gating or redaction failures found
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<outcome>
- Local verification passed on head `55efca7adfe436a2113519482a1fdb3b2ba6d0ef`: `make check`, `make lint`, and `make test` all completed successfully. This task did not change ultra-long test selection, so `make test-long` was not run locally.
- Authenticated hosted inspection used `/home/joshazimullah.linux/github-api-curl` against the `publish-images` workflow history for `djosh34/cockroach_migrate_tool` instead of guessing from YAML alone.
- The earliest still-downloadable failure was run 3 (`24639949135`, commit `401552158e3dca98169983935e2de8dd33cacc35`): the hosted `validate` job failed because the GitHub runner did not have `initdb` on `PATH`, and `bootstrap_contract` panicked with `initdb should start: No such file or directory (os error 2)`. The follow-up fix installed PostgreSQL tooling and exported the discovered PostgreSQL bin directory through `GITHUB_PATH`.
- GitHub had already pruned the downloadable job/log archives for intermediate failed runs 1, 2, 7, and 8, so this task does not invent exact stderr that could no longer be fetched. The surviving hosted history plus the adjacent workflow/test commits show the remaining real fixes that were applied while debugging hosted CI:
  - move the published-image manifest path behind a job-scoped env and then into `${{ github.workspace }}` so artifact upload/downstream steps use a GitHub-accepted workspace path
  - move each per-platform published-image ref file into `${{ github.workspace }}/published-image-refs/...` and create that directory before writing, so GitHub can upload the ref artifacts from native arm64/amd64 lanes
  - switch the publish topology to explicit native `ubuntu-24.04` and `ubuntu-24.04-arm` lanes so the real hosted arm64 path publishes instead of relying on wishful combined multi-arch emulation
- Hosted log inspection on successful run 11 (`24643444655`, commit `5fcfee60d11ca39e52830c5b6d9707114882b5f0`) confirmed the security and architecture behavior in real logs:
  - publish lanes logged `derived registry auth (masked): ***`, not raw credentials
  - native publish assertions logged `runner.arch=ARM64` with `publish platform=linux/arm64`
  - the manifest job also logged `derived registry auth (masked): ***` before downloading the six per-platform artifacts and publishing the canonical manifest
- The current-head hosted proof is run 13 (`24644253154`) for `55efca7adfe436a2113519482a1fdb3b2ba6d0ef`, which completed successfully from `2026-04-20T01:29:30Z` to `2026-04-20T01:45:46Z` with `validate-fast`, `validate-long`, all six `publish-image` jobs, and `publish-manifest` green.
- Trusted-secret usage remains gated to the intended `push` to `main` path through the repo-owned workflow contract boundary, and the hosted runs inspected for this task were real `main` push runs rather than untrusted PR or fork paths.
</outcome>

<plan>.ralph/tasks/story-21-github-workflows-image-publish/05-task-debug-real-github-image-build-failures-using-authenticated-workflow-api-log-access_plans/2026-04-20-real-github-image-failure-debug-plan.md</plan>
