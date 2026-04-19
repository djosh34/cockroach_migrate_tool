# Plan: Native ARM64 Under-Fifteen Publish Workflow

## References

- Task:
  - `.ralph/tasks/story-21-github-workflows-image-publish/02b-task-drive-three-image-github-pipeline-under-fifteen-minutes-with-native-arm64-and-workflow-restructure.md`
- Current workflow and workflow-contract boundary:
  - `.github/workflows/publish-images.yml`
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/github_workflow_contract.rs`
  - `crates/runner/tests/support/image_build_target_contract.rs`
- Prior speed-up task that this work supersedes:
  - `.ralph/tasks/story-21-github-workflows-image-publish/02-task-massively-improve-image-build-speed-with-docker-layer-and-build-cache-reuse.md`
  - `.ralph/tasks/story-21-github-workflows-image-publish/02-task-massively-improve-image-build-speed-with-docker-layer-and-build-cache-reuse_plans/2026-04-20-image-build-cache-speed-plan.md`
- Public runner reference verified on 2026-04-20:
  - GitHub-hosted Linux arm64 runners are documented with labels `ubuntu-24.04-arm` and `ubuntu-22.04-arm`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is approval for the workflow interface and behavior priorities in this turn.
- This turn is planning-only because the task file had no existing plan path or execution marker.
- The current workflow shape is already faster than before, but it still fails the new task because:
  - it builds `linux/arm64` on `ubuntu-latest`
  - it relies on QEMU/buildx emulation for the arm64 half of each publish
  - it still performs one two-platform build per image instead of splitting the platform work across native runners
- The previous completed task intentionally rejected native arm64 execution because no trusted runner label was known at that time.
  - This task exists specifically to reverse that decision using the now-verified GitHub-hosted native arm64 runner labels.
- The execution turn must use real hosted workflow evidence to decide whether the new topology is actually acceptable.
  - If the hosted end-to-end runtime for the real three-image pipeline is above fifteen minutes, this task stays `<passes>false</passes>`.
  - If hosted evidence shows the chosen runner label or topology is not actually available or not actually fast enough, execution must switch this plan back to `TO BE VERIFIED`.
- `make test-long` is still not a default end-of-task lane here.
  - Only run it if the workflow/test selection boundary for the ultra-long lane changes or the task evidence shows it is required.

## Current State Summary

- `.github/workflows/publish-images.yml` currently has one `validate` job, one `publish-image` matrix job, and one `publish-manifest` fan-in job.
- The current `publish-image` job runs on `ubuntu-latest` for every matrix entry and publishes both `linux/amd64` and `linux/arm64` from the same buildx invocation.
- The current workflow hard-codes the old arm64 decision:
  - `ARM64_BUILD_STRATEGY: emulated-buildx-qemu`
  - QEMU/binfmt installation
  - a step that explicitly says native arm64 is rejected
- `crates/runner/tests/support/github_workflow_contract.rs` currently enforces that old design directly.
- The workflow already has one good boundary worth preserving:
  - `ImageBuildTargetContract` owns the three-image identity, dockerfiles, contexts, artifact names, and cache scopes.
- The workflow still has a boundary smell from `improve-code-boundaries`:
  - image identity is centralized
  - platform/runner topology is not
  - the arm64 decision, runner labels, build mode, and platform strings are scattered between YAML and workflow-contract assertions

## Interface And Boundary Decisions

- Keep one canonical workflow file:
  - `.github/workflows/publish-images.yml`
- Keep `crates/runner/tests/ci_contract.rs` thin and behavior-focused.
  - It should continue to describe the public CI contract and delegate YAML parsing to `GithubWorkflowContract`.
- Preserve `ImageBuildTargetContract` as the single owner of image-specific facts:
  - image id
  - repository env
  - dockerfile
  - context
  - final manifest key
  - base cache scope
- Flatten the platform/runner topology into one honest workflow-contract boundary instead of sprinkling it through raw YAML checks.
  - Preferred direction:
    - keep image metadata where it already lives
    - add one narrow workflow-support owner for platform-specific expectations such as:
      - `linux/amd64`
      - `linux/arm64`
      - runner label
      - platform tag suffix
      - per-platform artifact name or digest file name
  - Avoid a second duplicate image-target registry just to represent the cartesian product.
- Replace the current one-build-does-both-platforms model with one platform-native build lane per platform.
  - Preferred topology:
    - `validate`
    - `publish-image` matrix with one entry per image/platform pair
    - `publish-manifest` fan-in that assembles the final multi-arch tag per image from the two platform-specific pushes
  - Required behavior:
    - amd64 entries run on `ubuntu-latest` or `ubuntu-24.04`
    - arm64 entries run on `ubuntu-24.04-arm`
    - no QEMU-based arm64 build path remains in the publish lane
- Publish platform-specific refs first, then create the canonical multi-arch commit-SHA tag in the manifest job.
  - This keeps native platform builds parallel and removes the old emulated two-platform build bottleneck.
- Make buildx installation architecture-aware.
  - The current direct install step hard-codes the amd64 buildx binary.
  - Execution must derive the right buildx artifact for the current runner architecture instead of assuming x64 everywhere.

## Public Contract To Establish

- One fast contract fails if any publish matrix entry still builds both `linux/amd64` and `linux/arm64` in a single invocation.
- One fast contract fails if the arm64 lane does not run on a native Linux arm64 hosted runner label.
- One fast contract fails if the arm64 lane still installs or enables QEMU/binfmt for publish.
- One fast contract fails if the publish lane no longer fans out by platform/image before the final manifest aggregation.
- One fast contract fails if the final manifest job does not depend on all platform-native publish work finishing first.
- One fast contract fails if the workflow stops publishing the canonical three-image set.
- One fast contract fails if platform-specific image refs/digests are not surfaced through a single honest artifact/output boundary that the manifest job consumes.
- One fast contract fails if buildx installation remains pinned to `linux-amd64` instead of following the runner architecture.
- One fast contract fails if the workflow stops using per-target remote BuildKit cache scopes for the publish path.
- One fast contract fails if trust boundaries regress while optimizing speed:
  - still `push` to `main` only
  - still explicit publish gating
  - still least-privilege permissions
  - still no outsider-controlled triggers
- One fast contract fails if README or workflow documentation still describes the old emulated arm64 path after execution.

## Improve-Code-Boundaries Focus

- Primary smell:
  - the repo has one honest owner for image identity but no equivalent honest owner for platform-native publish topology
- Required cleanup during execution:
  - remove `ARM64_BUILD_STRATEGY: emulated-buildx-qemu`
  - remove the QEMU-specific publish setup and rejection text
  - move platform-native assertions behind one workflow support boundary instead of raw YAML string drift
  - keep the workflow to one publish path, not one legacy emulated path plus one native path
- Smells to avoid:
  - adding separate `publish-amd64` and `publish-arm64` jobs with copy-pasted image definitions
  - adding a new helper that simply re-states `ImageBuildTargetContract`
  - locking the workflow tests to large raw shell snapshots instead of stable behavior-level checks
  - inventing platform tags, artifact names, or runner labels in multiple files

## Files And Structure To Add Or Change

- [ ] `.github/workflows/publish-images.yml`
  - replace the emulated two-platform build shape with native per-platform publication
  - install buildx in an architecture-aware way
  - remove QEMU/binfmt publish setup
  - push platform-specific refs first
  - assemble final multi-arch commit-SHA tags in `publish-manifest`
- [ ] `crates/runner/tests/ci_contract.rs`
  - replace the old explicit-emulation assertion with native-arm64 and fast-topology assertions
- [ ] `crates/runner/tests/support/github_workflow_contract.rs`
  - add the new platform-native workflow assertions
  - delete the old emulated-arm64 decision assertions
  - keep the helper as the one honest owner for workflow parsing
- [ ] `crates/runner/tests/support/image_build_target_contract.rs`
  - extend only if needed for shared per-platform artifact naming or cache-scope derivation
  - otherwise keep image identity here and keep platform topology elsewhere
- [ ] `README.md`
  - update the CI description only if it still claims the old emulated arm64 path
- [ ] No product runtime interfaces are expected to change
  - this task is workflow/test/doc contract work only

## TDD Execution Order

### Slice 1: Tracer Bullet For Native ARM64 Publish Topology

- [ ] RED: add one failing contract that requires the publish workflow to run the `linux/arm64` lane on `ubuntu-24.04-arm` and to stop using one `linux/amd64,linux/arm64` build invocation
- [ ] GREEN: make the smallest truthful workflow change that creates platform-native publish matrix entries and removes the old combined-platform publish command
- [ ] REFACTOR: keep platform runner/arch parsing inside `GithubWorkflowContract`, not inline in the test file

### Slice 2: Remove The Explicit Emulated ARM64 Boundary

- [ ] RED: add one failing contract that rejects:
  - `ARM64_BUILD_STRATEGY: emulated-buildx-qemu`
  - QEMU/binfmt installation in the publish lane
  - the old rejection text that claims no trusted native arm64 runner exists
- [ ] GREEN: remove the old emulated strategy boundary and replace it with a native-runner verification step that proves runner architecture matches the platform lane
- [ ] REFACTOR: keep the native-runner proof focused on observable workflow behavior, not exact shell wording

### Slice 3: Arch-Aware Buildx Installation

- [ ] RED: add one failing contract that requires publish dependency installation to derive the buildx download architecture from the runner instead of hard-coding `linux-amd64`
- [ ] GREEN: implement the minimum architecture-aware buildx installation and bootstrap path that works on both x64 and arm64 hosted runners
- [ ] REFACTOR: keep architecture mapping and install-step assertions owned by the workflow helper

### Slice 4: Preserve Fast Parallelism While Splitting By Platform

- [ ] RED: add one failing contract that requires the workflow to keep image publication parallelized after the native split:
  - matrix still covers all three images
  - platform-native entries can run independently
  - manifest aggregation remains a fan-in stage after the build fan-out
- [ ] GREEN: implement the minimum truthful topology that preserves parallel image publication while adding the native arm64 lane
- [ ] REFACTOR: remove any transitional serialized job shape instead of carrying both old and new topologies

### Slice 5: Publish Platform-Specific Refs And Rebuild The Final Multi-Arch Manifest

- [ ] RED: add one failing contract that requires the manifest stage to consume platform-specific publish outputs and create the final commit-SHA image refs for runner, setup-sql, and verify
- [ ] GREEN: add the minimum artifact/output boundary and manifest assembly logic needed to create the final multi-arch image refs from the native platform pushes
- [ ] REFACTOR: keep the final published image ref shape owned by the existing canonical image-target boundary

### Slice 6: Hosted Runtime Evidence Is The Real GREEN

- [ ] RED: after local contracts are green, use authenticated GitHub workflow inspection to measure a real hosted run of the three-image pipeline with the new topology
- [ ] GREEN: iterate until the real hosted run shows:
  - native arm64 execution for the arm64 lane
  - all three images published successfully
  - validation, build, and manifest gates still intact
  - full pipeline wall-clock runtime at or below fifteen minutes
- [ ] REFACTOR: if hosted evidence shows the topology or runner choice is wrong, switch the plan back to `TO BE VERIFIED` instead of papering over it

### Slice 7: Final Boundary Cleanup And Repo Lanes

- [ ] RED: if any contract/doc boundary still allows the repo to drift back toward emulated arm64 or re-serialized publication, add the smallest failing coverage needed
- [ ] GREEN: complete only the truthful doc/task updates after the required repo lanes and hosted evidence both pass
- [ ] REFACTOR: run one final `improve-code-boundaries` pass so:
  - image identity has one honest owner
  - platform-native workflow topology has one honest owner
  - there is no leftover emulated-arm64 publish branch in code, docs, or tests

## TDD Guardrails For Execution

- One failing test or hosted evidence slice at a time.
- Do not bulk-rewrite the workflow first and retrofit tests afterward.
- Do not keep the old emulated arm64 publish boundary around "just in case."
- Do not claim success from local timing guesses.
- Do not mark the task done without real hosted runtime evidence at or below fifteen minutes.
- Do not swallow hosted workflow failures, artifact handoff failures, manifest assembly failures, or runner-label failures.
- If GitHub-hosted native Linux arm64 turns out to be unavailable to this repository in practice, switch back to `TO BE VERIFIED` immediately.

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long` only if execution changes the ultra-long lane or proves it is required
- [ ] One final `improve-code-boundaries` pass after all required lanes are green
- [ ] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after the required lanes and hosted runtime evidence both pass

Plan path: `.ralph/tasks/story-21-github-workflows-image-publish/02b-task-drive-three-image-github-pipeline-under-fifteen-minutes-with-native-arm64-and-workflow-restructure_plans/2026-04-20-native-arm64-under-fifteen-plan.md`

NOW EXECUTE
