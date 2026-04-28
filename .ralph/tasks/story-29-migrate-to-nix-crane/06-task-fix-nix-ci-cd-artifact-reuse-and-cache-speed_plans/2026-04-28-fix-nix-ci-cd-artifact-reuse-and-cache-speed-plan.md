# Plan: Fix Nix CI/CD Artifact Reuse And Cache Speed

## References

- Task:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/06-task-fix-nix-ci-cd-artifact-reuse-and-cache-speed.md`
- Prior story steps:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only_plans/2026-04-28-migrate-ci-to-nix-only-plan.md`
- Current workflow and build surfaces:
  - `.github/workflows/publish-images.yml`
  - `.github/workflows/promote-image-tags.yml`
  - `.github/workflows/AGENTS.md`
  - `flake.nix`
- Authenticated hosted verification surface:
  - `/home/joshazimullah.linux/github-api-curl`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn started with no task-06 plan artifact, so this is a planning turn and must stop after the plan is written.
- This is a workflow task, so the `tdd` skill applies through real commands and hosted workflow behavior, not through fake YAML-string tests.
  - RED/GREEN slices must use `nix eval`, `nix build`, and authenticated GitHub Actions runs/logs.
  - Do not add Rust, shell, or snapshot tests that only assert workflow text.
- `.github/workflows/AGENTS.md` is correct about workflow verification:
  - local repo gates do not prove workflow correctness
  - hosted Actions evidence is the public contract for this task
- The task protocol still requires `make check`, `make lint`, and `make test` on the execution turn.
  - Treat those as repo-health regression gates only.
  - Treat hosted runs as the truthful proof that the workflow shape, cache behavior, and publish behavior are correct.
- `make test-long` remains out of scope for normal completion of this task.
- No backwards compatibility is allowed.
  - If the current workflow shape forces hidden rebuilds, delete or replace that shape.
  - Do not keep a fallback publish path that can rebuild images "just in case."
- If execution proves the workflow cannot hand off immutable Nix build identities between jobs without a different flake surface than planned here, switch this file back to `TO BE VERIFIED` immediately instead of hiding rebuilds behind best-effort cache restores.

## Hosted Baseline As Of 2026-04-28

- Latest `publish-images` run observed during planning:
  - run id `25034543962`
  - run number `86`
  - commit `be9871f5f7d00ebd4ec0f1cb5a56e3b276f7532f`
  - started `2026-04-28T04:50:30Z`
  - completed `2026-04-28T05:10:47Z`
  - wall-clock duration about `20m17s`
- Latest `publish-images` job timing snapshot from run `86`:
  - `publish-catalog`: `17s`
  - `validate-fast`: about `14m43s`
  - `validate-images`: about `6m33s`
  - four `publish-image` matrix jobs: about `3m00s` to `4m08s` each
  - `quay-security-gate`: about `25s`
  - `publish-manifest`: about `52s`
- Latest `promote-image-tags` run observed during planning:
  - run id `24678268137`
  - run number `1`
  - commit `5daf0fca70c75f5927d6df365f08586ed207da49`
  - started `2026-04-20T16:34:02Z`
  - completed `2026-04-20T16:34:21Z`
  - conclusion `failure`
- Current timing baseline already violates the task targets:
  - total wall clock is well above the required `10m`
  - the build-heavy path is well above the required `5m`

## Current State Summary

- `flake.nix` already owns image metadata through `github.publishImageCatalog`.
- The current `publish-images.yml` still has the wrong boundary for artifact reuse:
  - `publish-catalog` evaluates only the image catalog and publish matrix
  - `validate-fast` runs `nix run .#check`, `nix run .#lint`, and `nix run .#test` serially
  - `validate-images` runs `nix build --no-link .#runner-image .#verify-image`
  - every `publish-image` job then runs `nix build --no-link --print-out-paths ".#${package_attr}"` again before `skopeo copy`
- That means the workflow still treats the flake attr as the job handoff boundary instead of treating the already-built immutable output as the handoff boundary.
  - Even if Nix substitutes some outputs quickly, the workflow does not prove artifact reuse.
  - The publish stage still has a rebuild path available, which the task defines as failure.
- There is no workflow-level immutable identity audit yet.
  - No single manifest records `attr -> out path -> drv path -> nar hash -> published digest`.
  - No step fails if required evidence is missing.
- The current validation workflow shape is also muddy:
  - `check` and `lint` are backed by the same flake build target shape today
  - the workflow still pays for those surfaces separately
  - build-heavy work is split between `validate-fast`, `validate-images`, and each `publish-image` job without an explicit first-class artifact bundle boundary

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - the immutable Nix build result is not the thing that moves between workflow jobs
  - instead, the workflow keeps re-invoking flake attrs in downstream jobs and hoping Nix/cache behavior makes that cheap
- Desired boundary after execution:
  - the flake owns a CI catalog that tells GitHub exactly what to build, test, image, and publish
  - one small reusable helper owns exporting, importing, and auditing immutable Nix outputs
  - workflow YAML becomes orchestration only:
    - evaluate catalog
    - build once per platform
    - import exact outputs downstream
    - publish exact archives
    - fail if identity evidence is missing or duplicated
- Secondary boundary smell:
  - image publish metadata already lives in the flake, but build/test/image-stage selectors and audit identities do not
  - YAML still owns too much of the job graph shape and repeated Nix shell plumbing
- Desired cleanup:
  - extend the flake catalog so the workflow does not re-specify build/test/image bundles in multiple jobs
  - extract one honest helper for Nix store artifact transfer and identity manifests instead of duplicating long `nix` and `jq` blocks in several jobs
- Important non-goals:
  - do not create a pile of one-off shell scripts
  - do not keep package-attr-based rebuilds in publish as a fallback
  - do not treat missing hash/timing/cache evidence as success

## Proposed Public Workflow Interface

- Preferred new flake-owned CI surface:
  - `github.ciWorkflowCatalog`
  - or an equivalently clear top-level attr if a better name fits the existing flake style
- That catalog should own:
  - build platform matrix
  - image publish matrix
  - release image catalog for `promote-image-tags`
  - per-platform build bundle selectors
  - per-platform image bundle selectors
  - any static labels needed by the duplicate-build audit
- That catalog must not own:
  - secrets
  - branch protection logic
  - `github.sha`
  - run timestamps
- Preferred reusable helper boundary:
  - one script under `scripts/` or `.github/` only if it is genuinely reused by multiple workflow jobs
  - responsibilities:
    - build selected attrs once
    - record immutable identity metadata
    - export exact outputs for same-run handoff
    - import exact outputs in downstream jobs
    - fail loudly if required paths or metadata are missing
- Preferred immutable metadata per built output:
  - logical bundle id
  - platform/system
  - flake attr
  - output/store path
  - derivation path
  - nar hash or equivalent immutable Nix identity
  - later-published image digest if applicable

## Type And Interface Decisions

- Prefer one artifact bundle per platform stage instead of one bundle per tiny step.
  - That keeps the handoff boundary meaningful and avoids artifact sprawl.
- Prefer Nix-native export/import for same-run handoff.
  - The execution turn should try a truthful `nix copy` or `nix-store --export/--import` based bundle first.
  - If GitHub artifact transport makes the chosen Nix-native transfer unworkable, switch back to `TO BE VERIFIED` instead of quietly re-enabling rebuilds.
- Prefer workflow artifacts for same-run handoff and a Nix-native GitHub-hosted cache for reruns.
  - Same-run artifact reuse and cross-run cache reuse are different concerns and should stay separate in the design.
- Prefer one explicit audit manifest boundary.
  - Downstream jobs should consume a manifest emitted by the build or image stage rather than reconstructing identity from filenames.
- Prefer to remove workflow duplication around `check` vs `lint` if the workflow can use one honest flake-owned validation bundle instead.
  - Repo-level `make check` and `make lint` can stay as separate end-of-task gates.
  - The workflow does not need to pay twice for one build graph if the flake already makes them equivalent.

## TDD Execution Strategy

- This task uses the workflow/bootstrap exception form of TDD.
- Public contracts to test:
  - first-stage platform builds happen once and emit immutable output identities
  - test/check jobs reuse those outputs instead of rebuilding base artifacts
  - image jobs reuse those outputs and produce image archives once
  - publish jobs only consume already-built image archives and never invoke `nix build`
  - reruns substitute cached external dependencies instead of rebuilding them
  - missing evidence fails the workflow loudly
- Red-green workflow per slice:
  - RED:
    - use one real local `nix` command or one real hosted workflow run/log to expose the missing behavior
  - GREEN:
    - make the minimal flake/workflow/helper change needed for that one behavior
  - REFACTOR:
    - remove duplicated YAML/bootstrap/catalog logic only after the current slice is green
- Hosted verification requirements for execution:
  - inspect the exact run ids with `/home/joshazimullah.linux/github-api-curl`
  - record timestamps, job durations, cache/substitution evidence, output identities, and publish digests in the task file or execution notes
  - if required evidence is absent, fail the task instead of inferring success

## Vertical Execution Slices

### Slice 1: Flake-Owned CI Catalog Tracer Bullet

- [ ] RED:
  - run `nix eval --json .#github.ciWorkflowCatalog` and confirm it fails because the broader CI catalog does not exist yet
- [ ] GREEN:
  - add the minimal flake-owned CI catalog that covers build-platform, image, and release metadata
  - move at least one workflow consumer off ad hoc YAML-owned selectors onto that catalog
- [ ] REFACTOR:
  - keep `publish-images` and `promote-image-tags` on the same static metadata boundary
- Stop condition:
  - if the flake still cannot honestly own the build/test/image catalog without duplicating static data back into YAML, switch this plan back to `TO BE VERIFIED`

### Slice 2: Immutable Build-Bundle Helper

- [ ] RED:
  - try the planned local build/export command for one platform bundle and let it fail because no reusable helper or manifest boundary exists yet
- [ ] GREEN:
  - add one reusable helper that can:
    - build the selected attrs once
    - write immutable identity metadata
    - export the exact outputs for same-run reuse
    - import them later with hard failure on missing data
- [ ] REFACTOR:
  - keep the helper narrow and delete duplicated Nix-path bookkeeping from YAML

### Slice 3: First Build Stage Per Platform

- [ ] RED:
  - use the current hosted run shape as the failure: build-heavy work is split across `validate-fast`, `validate-images`, and downstream publish jobs with no explicit immutable handoff
- [ ] GREEN:
  - replace that with one first-stage matrix that builds the required platform-specific Nix artifacts exactly once per platform
  - emit a build manifest and upload the exported bundle as a workflow artifact
- [ ] REFACTOR:
  - delete `validate-images`
  - delete any downstream stage that still rebuilds the same first-stage attrs
- Design note:
  - if the existing flake does not expose the right pre-image or pre-test attrs for this boundary, execution should add those attrs explicitly instead of treating internal derivation knowledge as workflow contract

### Slice 4: Test/Check Stage Reuses First-Stage Outputs

- [ ] RED:
  - let the first downstream validation lane fail until it imports the first-stage bundle and proves reuse
- [ ] GREEN:
  - make the test/check stage depend on the build stage, import the exact bundle, and run the flake-owned validation surface against those imported outputs
- [ ] REFACTOR:
  - collapse any workflow-only duplication where `check` and `lint` are paying twice for the same graph
  - if needed, expose one clearer flake-owned CI validation surface rather than stacking identical workflow calls
- [ ] Verification:
  - hosted logs must show the downstream lane consuming imported or substituted outputs rather than rebuilding the heavy base artifacts

### Slice 5: Image Stage Builds Archives Once From Imported Base Outputs

- [ ] RED:
  - let the current image path fail the task because it does not have an explicit artifact-reuse boundary from stage one
- [ ] GREEN:
  - add an image stage that depends on the first build stage, imports the correct platform bundle, and builds every required image archive in parallel exactly once
  - emit immutable image-build metadata for each produced archive
- [ ] REFACTOR:
  - keep image-building responsibility in the image stage only
  - make publish consume image artifacts rather than flake attrs

### Slice 6: Publish Stage Only Publishes Prebuilt Image Archives

- [ ] RED:
  - make the publish stage fail if it still calls `nix build` or otherwise rebuilds an image archive
- [ ] GREEN:
  - publish by downloading the already-built image artifact and pushing it with `skopeo copy docker-archive:...`
  - record the resulting platform-tag digest and any final manifest digest in publish metadata
- [ ] REFACTOR:
  - remove package-attr-driven rebuild logic from publish jobs
  - keep only artifact download, copy, inspect, sign or push, and digest recording

### Slice 7: Duplicate-Build Audit And Loud Failure Modes

- [ ] RED:
  - let the workflow fail because no single audit manifest proves that each logical bundle/image was built once and published from the matching immutable identity
- [ ] GREEN:
  - add an audit step that merges build, image, and publish manifests and fails if:
    - a required identity field is missing
    - publish has no matching image-build record
    - one logical artifact shows multiple immutable identities inside the same run
    - required timing evidence is absent
- [ ] REFACTOR:
  - keep the audit on immutable identities, not on filename conventions or grep-based log heuristics

### Slice 8: Cross-Run Cache Proof And Timing Proof

- [ ] RED:
  - the current planning baseline already proves the pipeline misses the timing target and does not explicitly prove external dependency reuse
- [ ] GREEN:
  - add the chosen Nix-native GitHub-hosted cache integration
  - run hosted validation and record:
    - fixed-run id
    - unchanged rerun id
    - timestamps
    - job durations
    - substitution or restore evidence for external dependencies
    - immutable output identities reused across rerun
- [ ] Additional proof:
  - if the acceptance criteria still need a code-only change proof after the workflow fix lands, make one small honest source-only follow-up validation commit and record its run separately
  - that validation must show code-dependent artifacts rebuilding while already-built external dependencies stay substituted or restored
- [ ] REFACTOR:
  - remove any cache handling that is only hiding missing immutable-artifact handoff evidence

## Execution Guardrails

- Do not add tests that assert YAML text or log text fragments as the primary proof.
- Do not silently skip missing artifacts, missing hashes, or missing timestamps.
- Do not keep `nix build` in publish as a supposedly harmless fallback.
- Do not claim cache success from best-effort restore messages alone.
- Do not duplicate static CI graph metadata between `flake.nix` and workflow YAML.
- Do not let the helper become a second build system; it should only move and audit immutable Nix outputs.

## Expected File Shape After Execution

- `flake.nix`
  - expanded CI catalog and possibly new explicit build-bundle attrs if the current public outputs are too coarse
- `.github/workflows/publish-images.yml`
  - restructured into first-build, test/check, image, publish, and audit responsibilities with artifact reuse boundaries
- `.github/workflows/promote-image-tags.yml`
  - still consumes the same flake-owned release metadata boundary
- Likely one new reusable helper:
  - under `scripts/` or another narrow repo-owned location if it is truly reused by multiple workflow jobs
- Task notes:
  - task `06` markdown or execution notes must record the final run ids, durations, cache evidence, immutable output identities, and publish-no-rebuild proof

## Design Re-Verified

- The latest hosted workflow is functionally successful but structurally wrong for this task:
  - it is too slow
  - it still lets downstream jobs rebuild by attr instead of consuming exact immutable outputs
  - it does not emit the audit evidence the task requires
- The next turn should start with the smallest RED slice that introduces a flake-owned CI catalog and an immutable build-bundle handoff boundary.
- That is the clearest way to satisfy both `tdd` and `improve-code-boundaries` without inventing fake tests or more YAML duplication.

Plan path: `.ralph/tasks/story-29-migrate-to-nix-crane/06-task-fix-nix-ci-cd-artifact-reuse-and-cache-speed_plans/2026-04-28-fix-nix-ci-cd-artifact-reuse-and-cache-speed-plan.md`

NOW EXECUTE
