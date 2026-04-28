# Plan: Magic Nix Cache Matrix Workflow And Multi-Platform GHCR Assembly

## References

- Task:
  - `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/01-task-create-magic-nix-cache-matrix-workflow-and-combine-image-artifacts.md`
- Downstream task that depends on this workflow shape:
  - `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/02-task-publish-ghcr-built-multiplatform-images-to-quay-with-verbose-security-logs.md`
- Current build and package surfaces:
  - `flake.nix`
  - `Makefile`
- Existing script boundary:
  - `scripts/README.md`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`
- Current primary-source references to verify during execution:
  - GitHub Marketplace Magic Nix Cache usage:
    - `https://github.com/marketplace/actions/magic-nix-cache`
  - GitHub-hosted runner labels:
    - `https://docs.github.com/en/actions/writing-workflows/choosing-where-your-workflow-runs/choosing-the-runner-for-a-job`

## Planning Assumptions

- This turn is planning-only because the task file had no linked plan artifact yet.
- The task itself is the approval for the public workflow contract and verification priorities.
- TDD has an explicit exception here because this is workflow/infrastructure work.
  - Do not add brittle Rust or text-assertion tests for YAML content.
  - Use real executable boundaries instead:
    - local workflow syntax validation
    - local shell-script validation
    - real hosted GitHub Actions runs and authenticated log inspection
- The final execution turn must still follow red/green discipline.
  - RED means a real validation command or hosted workflow run fails.
  - GREEN means the smallest truthful repo change makes that real command or hosted run pass.
- Final execution must stop and switch this plan back to `TO BE VERIFIED` if any of these turn out false:
  - Magic Nix Cache now requires a different official action shape than the currently documented `DeterminateSystems/magic-nix-cache-action@main`.
  - GitHub-hosted `ubuntu-24.04-arm` runners are unavailable to this repository.
  - The current `dockerTools.buildImage` outputs cannot be consumed as no-rebuild OCI artifacts in the assembly job.

## Current State Summary

- There is currently no `.github/` directory, so this task will create the first workflow boundary instead of modifying an existing one.
- `flake.nix` already exposes the relevant package outputs on both supported systems:
  - `packages.x86_64-linux.runner-image`
  - `packages.aarch64-linux.runner-image`
  - `packages.x86_64-linux.verify-image`
  - `packages.aarch64-linux.verify-image`
- A quick Nix inspection confirmed the image outputs are `docker-image-*.tar.gz` store artifacts.
  - That is the correct shape for uploading per-architecture artifacts and later pushing them without a rebuild.
- `Makefile` already provides the required Nix-backed repo checks:
  - `make check`
  - `make lint`
  - `make test`
- Story-32 task 02 explicitly assumes this task produces one workflow with:
  - five parallel pre-assembly jobs
  - one dependent assembly/publish job
  - GHCR commit-SHA final tags for both images
- The repo already has a `scripts/` boundary.
  - That means complex image assembly logic should live in repo-owned CI scripts instead of growing into one giant inline YAML shell blob.

## Interface And Boundary Decisions

- Create one canonical workflow file:
  - preferred path: `.github/workflows/publish-images.yml`
- Trigger only on `push`.
  - No `concurrency` block.
  - No PR, tag, release, schedule, or workflow-chaining surface in this task.
- Use exactly five expensive/validation jobs before publish:
  - one `nix-flake-check` job
  - four matrix image-build jobs spanning:
    - `runner-image` + `amd64`
    - `runner-image` + `arm64`
    - `verify-image` + `amd64`
    - `verify-image` + `arm64`
- Use native GitHub-hosted Linux runners per architecture instead of emulation.
  - `amd64` jobs should use `ubuntu-24.04`
  - `arm64` jobs should use `ubuntu-24.04-arm`
- Install Nix and enable Magic Nix Cache in every job that runs Nix.
  - Keep the documented action order:
    - `actions/checkout`
    - Nix install action
    - Magic Nix Cache action
- Keep image-build jobs and the `nix flake check` job separate.
  - This keeps the five-way parallel topology obvious in GitHub UI and logs.
- Upload per-architecture image artifacts as the exact Nix-produced `docker-image-*.tar.gz` files plus a tiny metadata file per artifact.
  - Artifact names may include the image id and architecture.
  - Final published tags must not.
- Keep the complicated publish/assembly logic out of YAML string soup.
  - Add one repo-owned CI script under `scripts/ci/` for assembly/publish from downloaded archives.
  - Keep matrix-job build steps small and explicit in YAML.

## Improve-Code-Boundaries Focus

- Primary smell:
  - without a repo-owned script boundary, the final assembly job will become wrong-place shell soup inside YAML:
    - archive discovery
    - temp ref naming
    - per-arch push logic
    - manifest assembly
    - digest inspection
    - log formatting
- Desired boundary after execution:
  - workflow YAML owns orchestration only:
    - triggers
    - permissions
    - matrix
    - artifact upload/download
    - job dependencies
  - one repo-owned script owns the multi-step OCI assembly/publish behavior:
    - push each downloaded architecture archive to an internal temp GHCR ref
    - combine temp refs into the final multi-platform commit-SHA tag
    - print the final digest and platform list
- Concrete smells to avoid:
  - one 80+ line inline publish step inside YAML
  - duplicated shell fragments for runner and verify publish paths
  - hidden temp-ref naming rules spread across multiple steps
  - pushing final tags directly from matrix jobs

## Chosen Execution Design

- Workflow name and file:
  - `.github/workflows/publish-images.yml`
- Top-level permissions:
  - default to `contents: read`
  - do not grant `packages: write` globally
- `nix-flake-check` job:
  - install Nix
  - enable Magic Nix Cache
  - run `nix flake check --print-build-logs --show-trace`
  - use `set -euo pipefail`
- `build-images` matrix job:
  - matrix axes:
    - `image`: `runner-image`, `verify-image`
    - `arch`: `amd64`, `arm64`
    - `system`: `x86_64-linux`, `aarch64-linux`
    - `runs_on`: `ubuntu-24.04`, `ubuntu-24.04-arm`
  - install Nix
  - enable Magic Nix Cache
  - run the exact attr build:
    - `nix build .#packages.${system}.${image} --print-build-logs --show-trace`
  - copy the resulting `docker-image-*.tar.gz` store file into an artifact directory
  - write a metadata file capturing:
    - image id
    - architecture
    - system
    - intended temporary GHCR ref suffix
  - upload one artifact per matrix cell
- `publish-multiarch` job:
  - `needs` all five earlier jobs
  - grant `packages: write`
  - download all four artifacts
  - authenticate to GHCR without printing secrets
  - install the minimal OCI tooling needed for no-rebuild assembly
  - run one repo-owned script, tentatively:
    - `scripts/ci/publish-ghcr-multiarch-from-archives.sh`
  - script behavior:
    - read each downloaded archive + metadata pair
    - push each archive to a temporary architecture-specific GHCR ref using an OCI-aware tool such as `skopeo`
    - assemble the final multi-platform manifest for `runner-image:${GITHUB_SHA}` from the two temp refs
    - assemble the final multi-platform manifest for `verify-image:${GITHUB_SHA}` from the two temp refs
    - inspect and print the final digests and platform list for both published refs
- Temporary refs:
  - internal-only temp refs may use architecture suffixes
  - final published refs must use only `${GITHUB_SHA}`

## Files And Structures Expected To Change

- [x] `.github/workflows/publish-images.yml`
  - new workflow implementing the five-way parallel Nix/image pipeline plus the dependent publish job
- [x] `scripts/ci/publish-ghcr-multiarch-from-archives.sh`
  - repo-owned assembly/publish boundary for downloaded Nix image archives
- [x] `scripts/README.md`
  - mention the CI script boundary only if needed so future contributors know why the publish logic is not embedded in YAML
- [x] No application runtime code changes are expected
  - this task is workflow and CI-boundary work only

## Manual Red/Green Execution Order

### Slice 1: Tracer Bullet For Workflow Skeleton And Local Syntax

- [ ] RED:
  - add the new workflow file with the intended jobs, permissions, and no-concurrency shape
  - run local syntax validation and let it fail honestly if the YAML or expressions are wrong:
    - `nix shell nixpkgs#actionlint -c actionlint .github/workflows/publish-images.yml`
- [ ] GREEN:
  - fix the smallest syntax/shape issues until actionlint is green
- [ ] REFACTOR:
  - keep the workflow readable before any publish complexity lands

### Slice 2: Matrix Build Artifact Path

- [ ] RED:
  - run a local proof on the host architecture that the chosen matrix build command shape produces the expected `docker-image-*.tar.gz` artifact and metadata layout
  - if the artifact copy path or metadata contract is wrong, let that real command fail
- [ ] GREEN:
  - fix the artifact packaging shape so one matrix cell produces:
    - one non-empty archive file
    - one metadata file consumed later by publish
- [ ] REFACTOR:
  - keep artifact naming canonical in one place so runner and verify jobs do not drift

### Slice 3: Repo-Owned Assembly Script Boundary

- [ ] RED:
  - add the publish script and run it locally in the smallest honest way available
  - validate shell syntax and argument handling first
  - if feasible on the current host, use a local GHCR-disabled dry structure check that proves archive discovery and ref construction are coherent before remote publish
- [ ] GREEN:
  - make the script handle:
    - archive discovery
    - metadata parsing
    - temporary per-arch ref naming
    - final commit-SHA ref naming
    - digest/platform inspection output
- [ ] REFACTOR:
  - keep all image-loop logic in the script, not duplicated across workflow steps

### Slice 4: Hosted RED/GREEN For Real Workflow Behavior

- [ ] RED:
  - push the workflow branch and trigger the workflow through the normal `push` event
  - inspect the hosted run with `/home/joshazimullah.linux/github-api-curl`
  - treat any of these as real RED:
    - any Nix job missing Magic Nix Cache
    - any of the five required jobs not running in parallel
    - `nix flake check` serialized behind image jobs
    - artifact upload/download mismatch
    - final publish job starting before all five required jobs complete
    - final publish job rebuilding with Nix instead of using downloaded archives
    - final published tags not being exactly the commit SHA
    - digest/platform logging missing or unclear
- [ ] GREEN:
  - iterate on the workflow and publish script until the hosted run proves:
    - four image-build jobs plus `nix flake check` ran as separate parallel jobs
    - Magic Nix Cache was active in every Nix job
    - assembly used only downloaded artifacts and OCI tooling
    - GHCR contains one final multi-platform `runner-image:${sha}`
    - GHCR contains one final multi-platform `verify-image:${sha}`
- [ ] REFACTOR:
  - keep logs explicit enough that task 02 can extend the publish job without rediscovering the artifact contract

### Slice 5: Final Required Validation

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] local workflow syntax validation with `actionlint`
- [ ] authenticated hosted workflow evidence for:
  - five-way parallelism
  - Magic Nix Cache activity
  - final dependent publish job ordering
  - final GHCR digests and platform list
- [ ] final `improve-code-boundaries` pass to ensure the publish logic lives in the script boundary, not as inline YAML mud

## Execution Guardrails

- Do not add a `concurrency` block anywhere in the workflow.
- Do not add PR, release, or schedule triggers in this task.
- Do not build the final multi-platform tag with Nix.
  - Only per-architecture archive production may use Nix.
- Do not rebuild in the publish job.
  - The publish job must consume downloaded archives directly.
- Do not swallow errors with `|| true`, `continue-on-error`, or hidden fallbacks.
- Do not run `cargo`.
- Do not run `make test-long` for this task.
- If local validation exposes a need for a second helper script, add it only if it removes real duplication or wrong-place logic from the workflow.
  - Do not create generic helper-script clutter.

## Final Verification Checklist For The Execution Turn

- [ ] Workflow exists at `.github/workflows/publish-images.yml`
- [ ] Every Nix job installs Nix and enables Magic Nix Cache
- [ ] Exactly five pre-publish jobs run in parallel
- [ ] `nix flake check` is one of those five parallel jobs
- [ ] All Nix commands use `--print-build-logs --show-trace` and `set -euo pipefail`
- [ ] Each matrix job uploads a non-empty per-architecture image archive artifact
- [ ] The publish job depends on all five earlier jobs
- [ ] The publish job does not call Nix to rebuild images
- [ ] The publish job publishes exactly one final `runner-image:${sha}` and one final `verify-image:${sha}`
- [ ] Only the publish job has `packages: write`
- [ ] Hosted logs clearly show build commands, artifact names, assembly commands, final refs, and digests
- [ ] `make check` passes
- [ ] `make lint` passes
- [ ] `make test` passes

Plan path: `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/01-task-create-magic-nix-cache-matrix-workflow-and-combine-image-artifacts_plans/2026-04-28-magic-nix-cache-matrix-ghcr-plan.md`

NOW EXECUTE
