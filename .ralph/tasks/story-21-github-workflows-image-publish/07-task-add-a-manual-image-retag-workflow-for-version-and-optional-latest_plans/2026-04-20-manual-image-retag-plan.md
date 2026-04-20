# Plan: Manual Release Promotion For Published Images

## References

- Task:
  - `.ralph/tasks/story-21-github-workflows-image-publish/07-task-add-a-manual-image-retag-workflow-for-version-and-optional-latest.md`
- Current publish workflow and workflow-local instructions:
  - `.github/workflows/publish-images.yml`
  - `.github/workflows/AGENTS.md`
- Current public/operator image surface:
  - `README.md`
- Hosted verification path:
  - `github-api-auth-wrapper`
  - `/home/joshazimullah.linux/github-api-curl`
- Skills required during execution:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn is planning-only because the task had no existing `<plan>` path or execution marker.
- Workflow-local instructions override the generic local-test default for this task.
  - `.github/workflows/AGENTS.md` explicitly says not to invent local workflow tests and not to use `make check`, `make lint`, `make test`, or `make test-long` as proof for GitHub workflow behavior.
  - Execution verification for this task must therefore be hosted red/green through authenticated GitHub Actions API dispatch and run/log inspection.
- The current operator-facing published-image contract is GHCR, not Quay.
  - `README.md` teaches `ghcr.io/...:${IMAGE_TAG}` for runner, setup-sql, and verify.
  - `publish-images.yml` already treats Quay as the upstream publish-and-security gate and GHCR as the canonical public commit-SHA publication surface.
- The shared image-identity boundary is now concrete enough to execute.
  - Per-image repository names must be source-controlled, not live GitHub repository variables.
  - The canonical repository names should match the existing public README contract and prior story-21 plans:
    - `cockroach-migrate-runner`
    - `cockroach-migrate-setup-sql`
    - `cockroach-migrate-verify`
  - Quay keeps owning only the non-secret namespace boundary via `vars.QUAY_ORGANIZATION`.
  - GHCR keeps owning only the GitHub-owner namespace boundary via `${{ github.repository_owner }}`.
- To keep the release boundary honest and small, the manual retag workflow should promote the existing GHCR commit-SHA refs, not rebuild images and not reach back into Quay by default.
- The task description already fixes the public inputs:
  - required version string
  - boolean `set_latest`
- The source commit should stay implicit.
  - Dispatch the manual workflow on `master`.
  - Use the dispatched ref's current `github.sha` as the source commit-SHA tag to promote.
  - Do not add a separate source-SHA input unless hosted execution proves the "latest committed/pushed images" rule is too implicit or unsafe.
- If the first hosted slice proves that release promotion must tag Quay too, or must accept an explicit source SHA to stay truthful, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `.github/workflows/publish-images.yml` already publishes the full three-image set by image id:
  - runner
  - setup-sql
  - verify
- The automated publish workflow intentionally publishes exact full commit-SHA tags and intentionally does not publish `latest`.
- `.github/workflows/promote-image-tags.yml` now exists online as a separate manual workflow, but its first hosted dispatch already proved the image-reference boundary is wrong.
- The current story already distinguishes two boundaries:
  - Quay = trusted upstream publish plus security gate
  - GHCR = public/operator-facing image coordinates consumed by README and novice-user flows
- Boundary drift risk:
  - if this task duplicates or externalizes the image repository set without care, it will reintroduce stringly registry/repository drift
  - if this task tries to promote both Quay and GHCR in one manual flow without a strong reason, it will muddy the clean "security gate first, public release second" boundary that story 21 already established
- Hosted execution finding from 2026-04-20:
  - the first real `workflow_dispatch` run for `promote-image-tags` on commit `5daf0fca70c75f5927d6df365f08586ed207da49` failed before copy because the live repository variables currently resolve to bare names like `runner`, `setup-sql`, and `verify`
  - the current GHCR ref derivation in both the manual workflow and existing `publish-manifest` logic therefore points at invalid or untruthful names instead of the README contract, because it treats those mutable variables as the whole GHCR repository path
  - the checked-in source of truth is now strong enough to resolve that ambiguity:
    - `README.md` already teaches `ghcr.io/${GITHUB_OWNER}/cockroach-migrate-...:${IMAGE_TAG}`
    - prior story-21 planning already standardized the repository names to `cockroach-migrate-*`
    - the live failure proves the per-image repository variables are the wrong place for this knowledge
  - execution should therefore remove `RUNNER_IMAGE_REPOSITORY`, `SETUP_SQL_IMAGE_REPOSITORY`, and `VERIFY_IMAGE_REPOSITORY` from the public GHCR boundary instead of trying to repair those mutable values

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - release promotion is currently only an implied policy spread across task text, README expectations, and the absence of `latest` in the automated publish workflow
- Required cleanup during execution:
  - reduce config at the image-reference boundary:
    - own the canonical image repository names in one source-controlled image catalog
    - derive Quay refs as `quay.io/${QUAY_ORGANIZATION}/${image_repository}:${sha}`
    - derive GHCR refs as `ghcr.io/${{ github.repository_owner }}/${image_repository}:${tag}`
  - make release promotion an explicit, separate workflow boundary
  - keep automated publish responsible only for:
    - validation
    - raw Quay publication
    - Quay security evaluation
    - GHCR commit-SHA publication
  - keep the new manual workflow responsible only for:
    - GHCR release-tag promotion from already-published GHCR commit-SHA refs
    - optional `latest` promotion
    - operator-facing release summary
- Smells to avoid:
  - keeping per-image repository names in mutable repo variables when the repo already owns the contract
  - rebuilding from source in the manual flow
  - copying Quay-security logic into the manual flow
  - tagging both registries by default and thereby creating two public release surfaces
  - adding local Rust/YAML workflow tests that pretend to validate hosted workflow behavior
  - scattering the three-image target list across multiple shell fragments inside the new workflow

## Public Behavior To Establish

- A separate workflow exists solely for manual release-tag promotion.
- That workflow is `workflow_dispatch`-only and remains distinct from `.github/workflows/publish-images.yml`.
- The workflow accepts:
  - one required version input
  - one boolean `set_latest` input
- The workflow promotes the already-published GHCR commit-SHA refs for the dispatched `master` head commit to the requested version tag.
- When `set_latest` is `true`, the workflow also promotes those same image refs to `latest`.
- When `set_latest` is `false`, the workflow must not touch `latest`.
- The workflow must operate on the full published image set for this story:
  - runner -> `cockroach-migrate-runner`
  - setup-sql -> `cockroach-migrate-setup-sql`
  - verify -> `cockroach-migrate-verify`
- The workflow must fail loudly if:
  - the source commit-SHA image tag is missing
  - registry login fails
  - any copy/promotion step fails
  - post-copy inspection cannot confirm the promoted tag exists
- Hosted verification must dispatch the workflow through the GitHub API and prove the real run completed successfully, not just that the YAML looks plausible.

## Files Expected To Change During Execution

- [ ] `.github/workflows/promote-image-tags.yml`
  - keep the manual `workflow_dispatch` release-promotion flow
  - switch it to the canonical shared image catalog and explicit GHCR owner boundary
  - promote existing GHCR commit-SHA refs to version tags
  - optionally promote `latest`
  - keep permissions and trigger scope narrow
- [ ] `.github/workflows/publish-images.yml`
  - replace mutable per-image repository variables with the canonical shared image catalog
  - publish GHCR commit-SHA refs under the same canonical owner/package contract consumed by the manual promotion workflow
- [ ] `README.md`
  - only if one short operator-facing note is needed so the manual release-tag path is truthful and discoverable after the workflow boundary cleanup
- [x] No local workflow test files should be added
  - `.github/workflows/AGENTS.md` forbids fake local workflow testing for this work

## TDD Execution Order

### Slice 0: Re-Check Hosted Reality First

- [ ] RED: inspect the latest `publish-images` hosted runs and current workflow inventory through the authenticated GitHub API
- [ ] GREEN: confirm the checked-out task is still current, the publish workflow is the live source of commit-SHA refs, and no newer manual promotion workflow already exists online
- [ ] REFACTOR: if hosted reality contradicts the local repo shape in a material way, switch this plan back to `TO BE VERIFIED`

### Slice 1: Fix The Shared Image-Reference Boundary First

- [ ] RED: use the failed hosted `promote-image-tags` run plus the checked-in README/workflow code to prove the current GHCR coordinate derivation is wrong
- [ ] GREEN: introduce the smallest truthful shared image catalog across `publish-images.yml` and `promote-image-tags.yml` so:
  - the image repository names are source-controlled canonical values:
    - `cockroach-migrate-runner`
    - `cockroach-migrate-setup-sql`
    - `cockroach-migrate-verify`
  - Quay publication still derives only the namespace from `vars.QUAY_ORGANIZATION`
  - GHCR publication and promotion derive the namespace from `${{ github.repository_owner }}`
- [ ] REFACTOR: remove `RUNNER_IMAGE_REPOSITORY`, `SETUP_SQL_IMAGE_REPOSITORY`, and `VERIFY_IMAGE_REPOSITORY` from the public GHCR path boundary so this knowledge lives in one place

### Slice 2: Republish Correct GHCR Commit-SHA Source Refs

- [ ] RED: push the boundary fix to `master` and inspect the hosted `publish-images` run to see exactly where corrected GHCR publication still fails, if anywhere
- [ ] GREEN: iterate until `publish-manifest` publishes the three canonical GHCR commit-SHA refs for the pushed commit:
  - `ghcr.io/${owner}/cockroach-migrate-runner:${sha}`
  - `ghcr.io/${owner}/cockroach-migrate-setup-sql:${sha}`
  - `ghcr.io/${owner}/cockroach-migrate-verify:${sha}`
- [ ] REFACTOR: keep the image catalog owned in one obvious place rather than repeated across unrelated shell blocks or mutable repo settings

### Slice 3: Promote The Full GHCR Image Set To A Version Tag

- [ ] RED: dispatch `promote-image-tags` through `/home/joshazimullah.linux/github-api-curl` with a version input and `set_latest=false`, and let the first hosted failure reveal the minimum remaining promotion gap
- [ ] GREEN: keep or refine the smallest truthful direct-registry promotion path, preferably with `skopeo copy --all`, so the workflow:
  - logs in to GHCR
  - resolves each source image at the canonical `ghcr.io/${owner}/cockroach-migrate-*:${github.sha}` refs
  - promotes runner, setup-sql, and verify to the requested version tag without rebuilding
- [ ] REFACTOR: keep the three-image target list and the GHCR owner derivation on the same honest source-of-truth path

### Slice 4: Optional `latest` Promotion Must Be Real And Isolated

- [ ] RED: dispatch the workflow again through the GitHub API with `set_latest=true` and prove the current flow either misses `latest` or handles it in a muddy, non-obvious way
- [ ] GREEN: add the smallest truthful conditional branch so `latest` is promoted only when explicitly requested and never otherwise
- [ ] REFACTOR: keep version-tag promotion and optional `latest` promotion on the same source refs so there is one honest source-of-truth path

### Slice 5: Prove The Manual Workflow Does Not Rebuild Or Re-Validate

- [ ] RED: inspect the hosted manual run and prove whether the workflow accidentally performs source builds, repository test lanes, or other publish-workflow responsibilities
- [ ] GREEN: remove any accidental rebuild/test behavior so the workflow stays a pure registry-promotion path
- [ ] REFACTOR: keep the manual promotion workflow visibly smaller than the main publish workflow and clearly release-oriented in step naming and summary output

### Slice 6: Hosted Verification To Green

- [ ] RED: the task is not done until authenticated GitHub API dispatch and hosted run inspection prove:
  - manual dispatch succeeds on `master`
  - the requested version tag exists for all three GHCR images
  - `latest` is absent when `set_latest=false`
  - `latest` exists for all three GHCR images when `set_latest=true`
  - the workflow remained manual-only and separate from the main publish path
- [ ] GREEN: iterate on the manual workflow until the hosted run and registry inspection both match that contract
- [ ] REFACTOR: do one final `improve-code-boundaries` pass so release promotion remains one explicit GHCR boundary rather than bleeding back into Quay or the automated publish path

## Execution Guardrails

- Do not create local Rust/YAML workflow tests for this task.
- Do not claim success from local YAML inspection alone.
- Do not rebuild images, rerun repository validation, or duplicate the publish workflow in the manual promotion path.
- Do not broaden the main publish workflow triggers just to make manual release tagging easier.
- Do not swallow missing-source-tag, login, copy, or registry-inspection failures.
- Do not read, print, or expose any GitHub token; use `/home/joshazimullah.linux/github-api-curl`.
- If hosted verification proves the public release surface is not GHCR-only after all, switch this plan back to `TO BE VERIFIED` immediately instead of forcing the wrong release boundary.

## Next Step On Resume

- Execute Slice 0 and Slice 1 immediately.
- Treat the shared image catalog plus explicit GHCR owner namespace as settled design.
- Fix `publish-images.yml` and `promote-image-tags.yml` together so they both consume the same canonical package names before doing any more hosted dispatch verification.

Plan path: `.ralph/tasks/story-21-github-workflows-image-publish/07-task-add-a-manual-image-retag-workflow-for-version-and-optional-latest_plans/2026-04-20-manual-image-retag-plan.md`

NOW EXECUTE
