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
  - The image catalog must own two repository-name fields per image:
    - `quay_repository`
    - `ghcr_repository`
  - The truthful per-image values are now explicit enough to execute:
    - runner:
      - `quay_repository=runner`
      - `ghcr_repository=cockroach-migrate-runner`
    - setup-sql:
      - `quay_repository=setup-sql`
      - `ghcr_repository=cockroach-migrate-setup-sql`
    - verify:
      - `quay_repository=verify`
      - `ghcr_repository=cockroach-migrate-verify`
  - Quay keeps owning only the non-secret namespace boundary via `vars.QUAY_ORGANIZATION`.
  - GHCR keeps owning only the GitHub-owner namespace boundary via `${{ github.repository_owner }}`.
- To keep the release boundary honest and small, the manual retag workflow should promote the existing GHCR commit-SHA refs, not rebuild images and not reach back into Quay by default.
- The task description already fixes the public inputs:
  - required version string
  - boolean `set_latest`
- The source commit should stay implicit to the operator, but explicit inside the workflow.
  - Dispatch the manual workflow on `master`.
  - Resolve the source commit from the latest successful `publish-images` run on `master`, because that is the truthful publication record for GHCR commit-SHA refs.
  - Do not add a separate source-SHA input unless hosted execution proves GitHub's successful-run history is insufficiently precise.
  - `promote-image-tags.yml` will therefore need `actions: read` permission alongside its existing narrow permissions so it can query the Actions API for the latest successful publish run.
- If the first hosted slice proves that release promotion must tag Quay too, that latest-successful-run lookup is ambiguous or unavailable, or different Quay and GHCR repository names are still unresolved, switch this plan back to `TO BE VERIFIED` and stop immediately.

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
- Hosted execution finding from 2026-04-20 after the boundary-fix attempt:
  - commit `4878d0c16c5e899a9c3bda75654635cc27a1b96d` introduced a reusable source-controlled image catalog and pushed canonical `cockroach-migrate-*` names through the full publish path
  - hosted run `24678664756` accepted the reusable workflow and expanded the matrix with those canonical names, so the GHCR-side catalog shape is valid
  - the same run then failed in the verify publish jobs:
    - amd64 job `72169668730`
    - arm64 job `72169668711`
  - both failures were Quay push failures against `quay.io/cockroach_migrate_tool/cockroach-migrate-verify:*` with `401 UNAUTHORIZED`
  - this proves the design assumption "Quay repository name should equal the public GHCR package name" is false in the live environment
  - the source-controlled image catalog therefore needs separate registry-specific repository fields instead of one shared repository name
  - local history plus the earlier hosted dispatch failure are now enough to resolve the exact Quay-side repository names without more discovery:
    - the pre-refactor Quay publish path still read `matrix.image.repository_env`
    - the first hosted manual-dispatch failure already showed those repo variables resolve to bare names:
      - `runner`
      - `setup-sql`
      - `verify`
  - that gives one honest source-controlled mapping for the next execution turn:
    - `runner -> quay_repository=runner, ghcr_repository=cockroach-migrate-runner`
    - `setup-sql -> quay_repository=setup-sql, ghcr_repository=cockroach-migrate-setup-sql`
    - `verify -> quay_repository=verify, ghcr_repository=cockroach-migrate-verify`
  - a second design failure appeared locally at the same time:
    - `make test` failed in `readme_operator_surface_contract` because the operator quick-start README must not mention `workflow`
  - README is therefore the wrong place to document manual release-promotion process details
- Hosted execution finding from 2026-04-20 after the split-registry fix:
  - commit `9537a8d74e32c426eccc671fcaf1956360f87d6b` proved the split catalog is the right boundary for registry naming:
    - all six `publish-image` jobs passed
    - `validate-fast` and `validate-long` both passed
    - local `make check`, `make lint`, and `make test` also passed
  - the same run `24679069123` still did not reach GHCR commit-SHA publication because:
    - `quay-security-gate` timed out waiting for Quay scans to move beyond `queued`
    - `publish-manifest` was skipped
  - that disproves the execution assumption that the manual promotion workflow can safely infer its source as the dispatched ref's current `github.sha`
    - the latest pushed commit on `master` may exist
    - the same commit may have passed repository validation and raw Quay publication
    - yet the corresponding GHCR commit-SHA refs may still not exist when Quay security stalls
  - live GitHub API inspection on 2026-04-20 then confirmed there are currently zero successful `publish-images` runs on `master`
    - that means there is no truthful basis for "latest pushed commit equals latest published GHCR commit"
    - it also means the workflow must fail clearly when no successful publish run exists yet instead of pretending a dispatch ref is publishable
  - the manual promotion design is now explicit enough to resume:
    - keep the operator inputs as only `version` and `set_latest`
    - derive `source_sha` inside the workflow from the latest successful `publish-images` run on `master`
    - render that resolved `source_sha` in the job summary so the release boundary stays visible and auditable

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - release promotion currently mixes two different identities:
    - the workflow dispatch commit
    - the latest actually published GHCR commit image set
- Required cleanup during execution:
  - reduce config at the image-reference boundary:
    - own the registry-specific image repository names in one source-controlled image catalog
    - derive Quay refs as `quay.io/${QUAY_ORGANIZATION}/${quay_repository}:${sha}`
    - derive GHCR refs as `ghcr.io/${{ github.repository_owner }}/${ghcr_repository}:${tag}`
    - stop carrying any generic `image_repository` field across the workflow boundary because it muddies two different registries into one string
    - resolve the publish-ready source SHA once, in one dedicated lookup step, then reuse it everywhere
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
- The workflow resolves the latest successful `publish-images` run on `master` and promotes that run's already-published GHCR commit-SHA refs to the requested version tag.
- When `set_latest` is `true`, the workflow also promotes those same image refs to `latest`.
- When `set_latest` is `false`, the workflow must not touch `latest`.
- The workflow must operate on the full published image set for this story:
  - runner -> Quay `runner`, GHCR `cockroach-migrate-runner`
  - setup-sql -> Quay `setup-sql`, GHCR `cockroach-migrate-setup-sql`
  - verify -> Quay `verify`, GHCR `cockroach-migrate-verify`
- The workflow must fail loudly if:
  - no successful `publish-images` run exists on `master`
  - the source commit-SHA image tag is missing
  - registry login fails
  - any copy/promotion step fails
  - post-copy inspection cannot confirm the promoted tag exists
- The workflow summary must record the resolved published source SHA and the promoted image refs.
- Hosted verification must dispatch the workflow through the GitHub API and prove the real run completed successfully, not just that the YAML looks plausible.

## Files Expected To Change During Execution

- [ ] `.github/workflows/promote-image-tags.yml`
  - keep the manual `workflow_dispatch` release-promotion flow
  - switch it to the canonical shared image catalog and explicit GHCR owner boundary
  - resolve the source SHA from the latest successful `publish-images` run on `master`
  - promote existing GHCR commit-SHA refs to version tags
  - optionally promote `latest`
  - add only the minimum extra permission needed to read Actions run history
  - keep permissions and trigger scope narrow
- [ ] `README.md`
  - only if a wording change is needed that still keeps README operator-only and avoids release-process documentation
- [x] No local workflow test files should be added
  - `.github/workflows/AGENTS.md` forbids fake local workflow testing for this work

## TDD Execution Order

### Slice 0: Re-Check Hosted Reality First

- [x] RED: inspect the latest `publish-images` hosted runs and current workflow inventory through the authenticated GitHub API
- [x] GREEN: confirm the checked-out task is still current, the publish workflow is the live source of commit-SHA refs, and no newer manual promotion workflow already exists online
- [x] REFACTOR: if hosted reality contradicts the local repo shape in a material way, switch this plan back to `TO BE VERIFIED`

### Slice 1: Fix The Published-Source Boundary First

- [x] RED: use the failed hosted `promote-image-tags` run plus the zero-success live `publish-images` history to prove the current `${github.sha}` source assumption is wrong
- [x] GREEN: introduce the smallest truthful source-selection step inside `promote-image-tags.yml` so:
  - the workflow queries GitHub Actions for the latest successful `publish-images` run on `master`
  - it captures that run's `head_sha` as the one explicit `source_sha`
  - it fails clearly if no successful publish run exists
  - GHCR promotion uses `${source_sha}` instead of `${github.sha}`
- [x] REFACTOR: keep source selection in one dedicated step with one output instead of smearing publish-readiness logic across multiple shell blocks

### Slice 2: Republish Correct GHCR Commit-SHA Source Refs

- [x] RED: push the source-boundary fix to `master` and inspect the hosted `publish-images` run to see exactly where GHCR commit-SHA publication still fails, if anywhere
- [ ] GREEN: iterate until `publish-manifest` publishes the three canonical GHCR commit-SHA refs for the pushed commit:
  - `ghcr.io/${owner}/cockroach-migrate-runner:${sha}`
  - `ghcr.io/${owner}/cockroach-migrate-setup-sql:${sha}`
  - `ghcr.io/${owner}/cockroach-migrate-verify:${sha}`
- [ ] REFACTOR: keep the image catalog owned in one obvious place rather than repeated across unrelated shell blocks or mutable repo settings

### Slice 3: Promote The Full GHCR Image Set To A Version Tag

- [ ] RED: dispatch `promote-image-tags` through `/home/joshazimullah.linux/github-api-curl` with a version input and `set_latest=false`, and let the first hosted failure reveal the minimum remaining promotion gap
- [ ] GREEN: keep or refine the smallest truthful direct-registry promotion path, preferably with `skopeo copy --all`, so the workflow:
  - logs in to GHCR
  - resolves each source image at the canonical `ghcr.io/${owner}/cockroach-migrate-*:${source_sha}` refs selected from the latest successful publish run
  - promotes runner, setup-sql, and verify to the requested version tag without rebuilding
- [ ] REFACTOR: keep the three-image target list, the GHCR owner derivation, and the resolved `source_sha` on the same honest source-of-truth path

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
- If hosted verification proves Quay and GHCR do not share repository names, or proves README cannot hold release-process wording, switch this plan back to `TO BE VERIFIED` immediately instead of forcing the wrong boundary.
- Do not regress to using the dispatched workflow `github.sha` as a proxy for publication state; the only honest implicit source is the latest successful `publish-images` run on `master`.

## Next Step On Resume

- Continue from Slice 2 hosted execution.
- Keep the split registry catalog exactly as-is:
  - runner -> `quay_repository=runner`, `ghcr_repository=cockroach-migrate-runner`
  - setup-sql -> `quay_repository=setup-sql`, `ghcr_repository=cockroach-migrate-setup-sql`
  - verify -> `quay_repository=verify`, `ghcr_repository=cockroach-migrate-verify`
- Keep the new `source_sha` lookup in `promote-image-tags.yml` exactly as the single publish-ready source boundary.
- Push the current workflow fixes, wait for `publish-images` to complete, and verify that the longer Quay scan budget allows `publish-manifest` to publish the three GHCR commit-SHA refs.
- Then dispatch `promote-image-tags` twice through the GitHub API: once with `set_latest=false`, then once with `set_latest=true`.

Plan path: `.ralph/tasks/story-21-github-workflows-image-publish/07-task-add-a-manual-image-retag-workflow-for-version-and-optional-latest_plans/2026-04-20-manual-image-retag-plan.md`

NOW EXECUTE
