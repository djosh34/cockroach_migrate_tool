# Plan: Fix Nix CI/CD Artifact Reuse And Cache Speed

## References

- Task:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/06-task-fix-nix-ci-cd-artifact-reuse-and-cache-speed.md`
- Prior story steps that established the current Nix graph:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane_plans/2026-04-28-migrate-build-run-test-lint-to-crane-plan.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix_plans/2026-04-28-nix-image-generation-plan.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only_plans/2026-04-28-migrate-ci-to-nix-only-plan.md`
- Workflow and build surfaces in scope:
  - `.github/workflows/publish-images.yml`
  - `.github/workflows/promote-image-tags.yml`
  - `.github/workflows/AGENTS.md`
  - `flake.nix`
  - `scripts/nix_ci_artifacts.py`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`
  - `github-api-auth-wrapper`
- Relevant completed smell set:
  - `.ralph/tasks/smells/2026-04-28-story-29-ci-to-nix-boundaries.md`

## Planning Assumptions

- This turn started with no linked task-06 plan artifact, so this is a planning turn and must stop after the plan is linked and marked ready.
- This is a workflow/Nix task, so TDD must use real public behavior:
  - local `nix eval` and `nix build` for supporting flake surfaces
  - hosted GitHub Actions runs and logs for workflow proof
  - no fake YAML-string tests or Rust tests about workflow text
- The hard PO overrides in the task file take precedence over preserving the current workflow shape:
  - `scripts/nix_ci_artifacts.py` must be removed
  - the workflow must be fully rewritten rather than incrementally patched around the current bundle-script design
  - `DeterminateSystems/magic-nix-cache-action@v13` is the required GitHub-side cache mechanism
- The workflow-local rule in `.github/workflows/AGENTS.md` still applies:
  - do not treat `make check`, `make lint`, `make test`, or `make test-long` as proof that the hosted workflow works
  - hosted verification is the real public test boundary for this task
- Execution must fail loudly if hosted verification cannot be collected.
  - During planning, authenticated API probes against the current remote slug `djosh34/cockroach_migrate_tool` returned `404`.
  - The execution turn must resolve the accessible repository/workflow identifier first and stop rather than pretending hosted evidence exists.
- If execution proves the current flake shape cannot express truthful artifact-reuse evidence without reintroducing a second manual cache/control plane, switch this file back to `TO BE VERIFIED` immediately.

## Current State Summary

- `flake.nix` already owns the real build graph:
  - per-system `ci-build-bundle`
  - `runner-clippy`, `runner-test`, `verify-service-test`
  - `runner-image` and `verify-image`
  - a `github.ciWorkflowCatalog` output with build, validation, image, and audit metadata
- The current hosted workflow already uses Nix and `magic-nix-cache-action@v13`, but it still violates the task intent because it adds a second artifact/cache authority through `scripts/nix_ci_artifacts.py`.
- The current `publish-images.yml` still does all of the following through the repo-maintained Python helper:
  - `build-bundle`
  - `import-bundle`
  - `assert-no-duplicate-build-plan`
  - `audit-run`
- The current publish pipeline still moves large build state through uploaded bundle directories instead of letting Nix reuse the store through the cache/native installables boundary.
- The current image publish path is only partially truthful:
  - it publishes a prebuilt `docker-archive` with `skopeo copy`
  - but it first reconstructs/imports the bundle through the Python helper
  - that leaves the workflow responsible for a cache layer that Nix should own
- Task 04 notes show the earlier Nix-only workflow succeeded in hosted run `#78`, but the current tree has drifted into a slower script-heavy design and now fails the stricter task-06 requirements.

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - the flake owns the build graph
  - the workflow owns a second "artifact bundle" graph through `scripts/nix_ci_artifacts.py`
  - that splits authority for reuse, auditing, and cache behavior across Nix and ad hoc workflow logic
- Desired boundary after execution:
  - `flake.nix` owns installables, image outputs, and static workflow matrix metadata
  - GitHub Actions only orchestrates:
    - install Nix
    - enable Magic Nix Cache
    - run direct `nix build`/`nix eval` commands
    - upload only the small evidence artifacts and the final built image archives
    - publish already-built image archives without any `nix build` in publish
- Secondary boundary smell:
  - current workflow audit logic depends on a repo script that mirrors Nix store identity concepts in Python
- Desired cleanup:
  - use Nix-native identities directly:
    - out paths
    - derivation/output paths
    - nar hashes
    - final registry digests
  - store those as small JSON artifacts emitted by the jobs themselves
  - collate them in one workflow audit job without re-packing the Nix store
- Important non-goals:
  - do not introduce a replacement helper script under `scripts/`
  - do not preserve the bundle/import design behind a different filename
  - do not add workflow tests that only grep YAML text

## Proposed Public Workflow Interface

- Flake-owned static data:
  - one minimal GitHub workflow catalog stays in `flake.nix`
  - it should own only:
    - platform matrix
    - validation lane installables
    - image installables
    - static image metadata such as `image_id`, loaded archive ref, repository names, and manifest keys
  - it should stop owning bundle ids, bundle artifact names, or any workflow-only cache-import metadata
- Stage 1: `build-platform`
  - matrix by platform/system
  - runs direct `nix build --no-link` for the platform foundation installables
  - emits a small JSON artifact containing:
    - system
    - installables built
    - out paths
    - nar hashes
    - build log artifact path or summary references
- Stage 2a: `validate`
  - depends on `build-platform`
  - runs direct `nix build --no-link` for the validation installables
  - must reuse Stage-1 outputs through Nix store/cache substitution rather than manual bundle import
  - emits JSON evidence for output identities and the structured Nix log summary
- Stage 2b: `build-image`
  - depends on `build-platform`
  - runs in parallel with `validate`
  - runs direct `nix build --no-link` for each image installable
  - uploads the resulting archive path as the image artifact plus a JSON identity record
- Stage 3: `publish-image`
  - depends only on `build-image`
  - downloads the already-built image archive artifact
  - publishes it with `skopeo copy docker-archive:... docker://...`
  - records final per-platform refs and digests
  - must not call `nix build`
- Stage 4: `audit-immutable-identities`
  - depends on `build-platform`, `validate`, `build-image`, and `publish-image`
  - downloads only JSON/image artifacts from previous jobs
  - fails if required outputs are missing or if identity changes show a duplicate/rebuilt artifact path
- Stage 5: `publish-manifest`
  - depends on `publish-image` and `audit-immutable-identities`
  - creates multi-arch manifests only from the published per-platform refs
  - must not build or republish image contents

## Type And Interface Decisions

- Keep one flake-evaluable catalog, but flatten it from "workflow bundle plan" into "direct installable matrix + static image metadata".
- Prefer JSON emitted by Nix tooling and tiny inline shell/Python/JQ in the workflow over repo-maintained helper scripts.
  - inline metadata fan-in is acceptable
  - manual cache packing, cache parsing, or store import/export helpers are not
- Preserve these static image metadata fields in the flake catalog because they are honest image identity, not workflow state:
  - `image_id`
  - `package_attr`
  - `loaded_image_ref`
  - `quay_repository`
  - `ghcr_repository`
  - `manifest_key`
- Remove workflow-only fields that exist solely because of the deleted bundle helper:
  - bundle ids
  - bundle artifact names
  - bundle manifest paths
  - required build/publish output ids that mirror the helper's internal model instead of the Nix outputs
- Prefer `nix path-info --json` and Nix internal JSON logs as the evidence source for reuse.
  - If those logs cannot distinguish local build vs substitution honestly enough, execution must stop and redesign rather than inventing a fake heuristic.

## TDD Execution Strategy

- Use vertical workflow slices with real behavior, not a horizontal rewrite followed by "hope".
- For each slice:
  - RED:
    - demonstrate the current public behavior is wrong using a direct `nix` command, current workflow inspection, or a hosted run/log
  - GREEN:
    - make the smallest truthful flake/workflow change to remove that one failure
  - REFACTOR:
    - delete the now-obsolete script/config duplication before moving to the next slice
- Hosted verification rules:
  - use `/home/joshazimullah.linux/github-api-curl` exactly as a normal `curl` wrapper
  - record the exact repo slug/workflow identifier that works
  - record run ids, timestamps, job durations, cache/substitution evidence, output identities, image digests, and no-rebuild publish proof in the task notes

## Vertical Execution Slices

### Slice 1: Tracer Bullet On The Wrong Boundary

- [ ] RED:
  - prove the current workflow still depends on `scripts/nix_ci_artifacts.py`
  - run one direct flake command such as `nix eval --json .#github.ciWorkflowCatalog` and show the catalog still contains workflow-bundle-specific structure
- [ ] GREEN:
  - redesign the flake catalog to expose direct installable matrices and static image metadata only
  - remove the first workflow dependency on the Python helper
- [ ] REFACTOR:
  - delete `scripts/nix_ci_artifacts.py` once no workflow step depends on it
  - remove any flake fields that only existed for that helper
- Stop condition:
  - if direct installable matrices are not expressive enough and the workflow needs a second artifact-control plane, switch back to `TO BE VERIFIED`

### Slice 2: Platform Build Stage Becomes Pure Nix + Cache

- [ ] RED:
  - run the first platform-build path locally with direct `nix build --no-link` and show what the workflow currently has to reconstruct through bundles
- [ ] GREEN:
  - rewrite the first workflow stage to build the platform foundation installables directly with Nix and Magic Nix Cache
  - emit only small identity/log artifacts, not bundle directories
- [ ] REFACTOR:
  - remove bundle upload/download/import logic from that stage completely

### Slice 3: Validation Reuses Platform Outputs Without Bundle Import

- [ ] RED:
  - let one validation lane fail honestly until it uses direct `nix build --no-link` on top of the new platform-build stage
- [ ] GREEN:
  - validate through Nix using the flake-defined installables only
  - collect Nix path/log evidence showing reused outputs instead of second builds
- [ ] REFACTOR:
  - delete `assert-no-duplicate-build-plan`-style logic and replace it with direct identity/log evidence from Nix

### Slice 4: Image Builds Reuse Platform Outputs And Stay Parallel

- [ ] RED:
  - move one image/platform axis onto the new direct build flow and let any remaining bundle assumption fail honestly
- [ ] GREEN:
  - build each image archive directly with `nix build --no-link`
  - upload the built archive and a JSON identity record
  - run all image/platform axes in parallel
- [ ] REFACTOR:
  - remove the remaining bundle-manifest and import plumbing from image jobs

### Slice 5: Publish Consumes Only Built Archives

- [ ] RED:
  - make the publish job fail honestly if it still tries to rebuild or import through Nix instead of consuming downloaded image archives
- [ ] GREEN:
  - publish with `skopeo copy` from downloaded image archives only
  - record final refs and digests as JSON artifacts
- [ ] REFACTOR:
  - ensure publish has no `nix build` or helper-script dependency left at all

### Slice 6: Identity Audit Without A Repo Helper

- [ ] RED:
  - let the audit job fail honestly once the helper script is gone and no direct identity collation exists yet
- [ ] GREEN:
  - collate the per-job JSON evidence and fail on:
    - missing required outputs
    - changed out paths/nar hashes where reuse was required
    - missing publish digests or manifest inputs
  - keep the audit logic inline in the workflow unless it becomes obviously reusable and truthful as a flake-owned command surface
- [ ] REFACTOR:
  - remove all references to helper-owned ids or manifests that no longer exist

### Slice 7: Hosted Rerun Proof For Cache Reuse And Timing

- [ ] RED:
  - identify the first hosted run that still rebuilds external dependencies, lacks substitution evidence, exceeds timing bounds, or has inaccessible logs
- [ ] GREEN:
  - push the rewritten workflow
  - run hosted validation on the protected workflow path
  - use authenticated GitHub API/log inspection to confirm:
    - unchanged rerun does not rebuild external dependencies
    - a code-only rerun rebuilds only code-dependent artifacts
    - publish performs no rebuild
    - build-heavy work is 5 minutes or less
    - end-to-end wall clock is 10 minutes or less
- [ ] REFACTOR:
  - remove any leftover workflow noise that exists only to support the old bundle design
- Stop condition:
  - if hosted logs cannot be accessed or do not expose enough truthful Nix evidence, switch back to `TO BE VERIFIED` instead of guessing

## Execution Guardrails

- Do not add or keep any repo-maintained caching script, Python helper, or manual Nix store import/export path.
- Do not replace `scripts/nix_ci_artifacts.py` with another file in `scripts/`.
- Do not hide any missing evidence behind `continue-on-error`, ignored shell failures, or optional parsing.
- Do not let publish jobs call `nix build`.
- Do not keep two competing audit models:
  - helper-script ids on one side
  - real Nix output identities on the other
- Do not treat local repo lanes as sufficient proof for workflow behavior.
- If a real bug is found that cannot be fixed in this task, create the required bug task rather than swallowing it.

## Expected File Shape After Execution

- `flake.nix`
- `.github/workflows/publish-images.yml`
- `.github/workflows/promote-image-tags.yml`
- `scripts/nix_ci_artifacts.py` deleted
- No replacement helper script introduced for workflow caching/artifact transport

## Expected Outcome

- The workflow becomes a thin orchestration layer over the existing flake build graph.
- Artifact reuse is driven by Nix store identity and Magic Nix Cache rather than repo-maintained bundle logic.
- Validation and image jobs reuse the first-stage platform builds without manual import steps.
- Publish uses only already-built archives and proves no rebuild happened.
- Hosted logs and JSON evidence make duplicate builds, cache misses, and timing regressions visible and fatal.

## Design Re-Verified

- The core design change is correct:
  - delete the bundle helper boundary entirely
  - keep the flake as the single build graph
  - use direct Nix commands plus cache-backed hosted verification as the public contract
- The execution turn should begin by flattening the flake/workflow interface, not by polishing the current Python helper path.

Plan path: `.ralph/tasks/story-29-migrate-to-nix-crane/06-task-fix-nix-ci-cd-artifact-reuse-and-cache-speed_plans/2026-04-28-fix-nix-ci-cd-artifact-reuse-and-cache-speed-plan.md`

NOW EXECUTE
