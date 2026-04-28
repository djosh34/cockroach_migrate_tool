# Plan: Replace Determinate Magic Nix Cache With Cachix In The Publish Workflow

## References

- Task:
  - `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/03-task-replace-magic-nix-cache-with-cachix.md`
- Current workflow:
  - `.github/workflows/publish-images.yml`
- Existing publish-script boundary that must remain untouched unless the workflow contract forces a change:
  - `scripts/ci/publish-ghcr-multiarch-from-archives.sh`
  - `scripts/ci/publish-quay-from-ghcr.sh`
- Repo validation entrypoints:
  - `Makefile`
- Hosted workflow inspection path:
  - `/home/joshazimullah.linux/github-api-curl`
- Primary-source action/version checks required during execution:
  - `actions/download-artifact`
  - `cachix/install-nix-action`
  - `cachix/cachix-action`
  - GitHub Actions runner/runtime compatibility docs where needed
- Required skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn is planning-only because the task had no `<plan>` pointer yet.
- This task is a TDD exception because it is workflow/configuration work, not application code.
  - Do not add brittle tests that assert YAML strings.
  - Use real executable validation boundaries instead:
    - local workflow linting with `actionlint`
    - final repo quality lanes via `make check`, `make lint`, and `make test`
    - a real hosted GitHub Actions run inspected with authenticated log access
- The task markdown is treated as approval for the public workflow contract and verification priorities.
- No local `devenv.nix` exists in this repo today.
  - Execution must not introduce one just because generic Cachix instructions mention `devenv`.
- The publish scripts are already the right boundary for image assembly/publish behavior.
  - This task should stay focused on Nix/cache bootstrap and workflow wiring unless execution proves the workflow contract is incomplete.

## Current State Summary

- `.github/workflows/publish-images.yml` currently installs Nix with `DeterminateSystems/nix-installer-action@main` in:
  - `nix-flake-check`
  - `build-images`
- The same workflow currently enables `DeterminateSystems/magic-nix-cache-action@main` in both of those Nix-running jobs.
- Both Nix-running jobs currently grant `id-token: write`.
  - Based on the current workflow, that permission appears to exist only for Determinate/FlakeHub-related setup and should be removable once Cachix replaces it.
- The publish job currently uses `actions/download-artifact@v4`.
  - The task requires bumping this to the newest stable major that supports GitHub's Node.js 24 runtime, verified at execution time from upstream sources instead of guessed.
- The current failure modes are workflow/bootstrap failures, not application failures:
  - Magic Nix Cache rate limiting
  - FlakeHub login failure from `determinate-nixd`
- The existing image-build and publish boundaries should remain the same:
  - Nix jobs build and upload archives
  - publish job downloads existing artifacts and publishes registries
  - no rebuild in the final publish job

## Improve-Code-Boundaries Focus

- Primary boundary problem:
  - vendor-specific Nix bootstrap and cache setup knowledge is duplicated directly inside multiple jobs, which mixes workflow orchestration with cache-provider implementation details
- Desired boundary after execution:
  - workflow jobs own only:
    - permissions
    - job topology
    - matrix/build commands
    - artifact flow
  - one local reusable setup boundary owns:
    - Nix installation choice
    - Cachix cache name
    - Cachix token wiring
    - any shared extra Nix configuration required by both Nix-running jobs
- Preferred implementation:
  - add a local composite action at `.github/actions/setup-nix-cachix/action.yml`
  - use it from `nix-flake-check` and `build-images`
- Smells to avoid:
  - repeating `cachix/install-nix-action` and `cachix/cachix-action` blocks in every Nix job
  - dragging publish-job logic into the new setup boundary
  - creating a fake helper that wraps only one job or needs job-specific conditionals to stay alive
- Stop condition for this boundary choice:
  - if the composite action becomes muddier than the workflow itself or needs per-job branching, inline the setup cleanly in the workflow instead of preserving a bad abstraction

## Intended Public Contract After Execution

- `.github/workflows/publish-images.yml` no longer references:
  - `DeterminateSystems/nix-installer-action`
  - `DeterminateSystems/magic-nix-cache-action`
  - FlakeHub login/cache touchpoints in the Nix-running jobs
- The Nix-running jobs use a non-Determinate installation path compatible with Cachix.
- The Nix-running jobs configure Cachix cache `djosh34` and authenticate with `${{ secrets.CACHIX_TOKEN }}`.
- `id-token: write` is removed from jobs where it only existed for Determinate/FlakeHub auth.
- `actions/download-artifact` is updated to the verified stable major that supports the current Node.js 24 GitHub Actions runtime.
- No unrelated local-development cache files are added.
- A real hosted workflow run proves:
  - no Magic Nix Cache rate-limit warnings
  - no FlakeHub login failures
  - Cachix is configured in the Nix jobs
  - the existing image build and publish flow still works end to end

## Files Expected To Change

- [ ] `.github/workflows/publish-images.yml`
  - replace Determinate bootstrap/cache steps with the chosen Cachix-compatible setup
  - trim permissions
  - bump `actions/download-artifact`
- [ ] `.github/actions/setup-nix-cachix/action.yml`
  - new local composite action if it keeps the boundary cleaner than inline duplication
- [ ] `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/03-task-replace-magic-nix-cache-with-cachix.md`
  - add execution notes, hosted evidence, and final checkbox state after implementation
- [ ] No application code or publish-shell script changes are expected by default
  - only expand scope if hosted validation proves the current publish contract depends on Determinate-specific side effects

## Execution Slices For The Next Turn

### Slice 1: Verify Upstream Action Versions Before Editing

- [ ] Check the current stable upstream version guidance for:
  - `actions/download-artifact`
  - `cachix/install-nix-action`
  - `cachix/cachix-action`
- [ ] Use official upstream sources only and record the exact chosen versions in task notes.
- [ ] Treat any ambiguity about the latest stable Node.js 24-compatible `actions/download-artifact` major as a blocker that must be resolved from the upstream action docs/releases before editing.
- Stop condition:
  - if upstream docs/releases conflict or do not clearly show the supported stable version, switch this plan back to `TO BE VERIFIED`

### Slice 2: Replace The Workflow Bootstrap Boundary

- [ ] Add the local `setup-nix-cachix` composite action if it is the cleanest way to centralize Nix + Cachix setup.
- [ ] Otherwise, inline the Cachix-compatible setup directly in the workflow and explicitly note why the helper boundary was rejected.
- [ ] Update `nix-flake-check` to use the new bootstrap path.
- [ ] Update `build-images` to use the new bootstrap path.
- [ ] Remove `DeterminateSystems/nix-installer-action`.
- [ ] Remove `DeterminateSystems/magic-nix-cache-action`.
- [ ] Remove any `use-flakehub` or other FlakeHub/Determinate login-cache configuration from the workflow.
- [ ] Remove `id-token: write` from affected jobs if it is no longer required after the bootstrap swap.
- [ ] Run local workflow linting immediately after these edits:
  - `nix shell nixpkgs#actionlint -c actionlint .github/workflows/publish-images.yml`
- Stop condition:
  - if the Cachix action setup requires materially different permissions, environment layout, or runner assumptions than planned here, switch back to `TO BE VERIFIED`

### Slice 3: Bump Artifact Download Action And Re-Validate

- [ ] Update `actions/download-artifact` in the publish job to the verified stable major from Slice 1.
- [ ] Re-run:
  - `nix shell nixpkgs#actionlint -c actionlint .github/workflows/publish-images.yml`
- [ ] Keep the publish job behavior unchanged apart from the verified action version bump.
- Stop condition:
  - if the newer `download-artifact` major requires workflow-contract changes that affect artifact layout or publish semantics, switch back to `TO BE VERIFIED`

### Slice 4: Hosted Red/Green Verification

- [ ] Push the workflow changes through the normal GitHub `push` trigger.
- [ ] Inspect the hosted run with `/home/joshazimullah.linux/github-api-curl`.
- [ ] Treat any of the following as real RED:
  - Magic Nix Cache still appears in the logs
  - FlakeHub login failures still appear
  - Cachix setup/authentication fails
  - the Nix jobs do not show Cachix usage/configuration
  - the workflow no longer reaches a successful publish path after the cache/bootstrap swap
- [ ] If Cachix authentication fails:
  - do not work around it silently
  - create a bug task with the `add-bug` skill
  - record the exact hosted failure evidence in task notes
  - leave the task honest rather than pretending success
- [ ] Hosted success evidence to capture:
  - exact workflow run URL or run id
  - proof that there are no Magic Nix Cache rate-limit warnings
  - proof that there are no FlakeHub login failures
  - proof that Cachix was configured for the Nix jobs
  - proof that the overall publish workflow still completed successfully

### Slice 5: Final Repository Validation And Boundary Review

- [ ] Run `make check`
- [ ] Run `make lint`
- [ ] Run `make test`
- [ ] Do not run `make test-long`
- [ ] Final `improve-code-boundaries` review:
  - Nix/cache vendor details live in one clear boundary
  - workflow YAML remains orchestration-focused
  - no swallowed errors or fallback hacks such as `|| true`

## Planned Validation Commands

- `nix shell nixpkgs#actionlint -c actionlint .github/workflows/publish-images.yml`
- `make check`
- `make lint`
- `make test`

## Expected Outcome

- The publish workflow stops depending on Determinate and Magic Nix Cache.
- The FlakeHub login failure path disappears because the workflow no longer uses the Determinate installer/cache path there.
- Cachix becomes the single explicit binary-cache integration for the Nix-running jobs.
- The workflow stays cleaner than a raw inline migration because the cache/bootstrap boundary is centralized instead of duplicated.

Plan path: `.ralph/tasks/story-32-github-workflow-multiplatform-image-artifacts/03-task-replace-magic-nix-cache-with-cachix_plans/2026-04-28-cachix-workflow-replacement-plan.md`

NOW EXECUTE
