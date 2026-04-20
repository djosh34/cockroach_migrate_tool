# Plan: Three-Image Master-Only Publish Workflow

## References

- Task:
  - `.ralph/tasks/story-21-github-workflows-image-publish/01-task-fix-github-workflows-to-build-test-and-publish-the-three-image-split.md`
- Existing workflow and workflow-contract boundary:
  - `.github/workflows/master-image.yml`
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/github_workflow_contract.rs`
- Existing published-image boundary:
  - `crates/runner/tests/support/published_image_contract.rs`
  - `crates/runner/tests/support/readme_published_image_contract.rs`
- Existing image build contracts:
  - `Dockerfile`
  - `crates/setup-sql/Dockerfile`
  - `cockroachdb_molt/molt/Dockerfile`
  - `crates/runner/tests/image_contract.rs`
  - `crates/setup-sql/tests/image_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
- Existing novice-user and registry contract:
  - `README.md`
- Skill:
  - `tdd`
- Skill:
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public workflow behavior and test priorities in this turn.
- This turn is planning-only because the task file had no existing plan path or execution marker.
- The current workflow/test boundary is obsolete for this task:
  - one `master-image.yml`
  - two images only
  - hard-coded `master`
  - several convenience actions where direct shell installation/login/build steps are practical
- The real product split is now three publishable images:
  - runner
  - setup-sql
  - verify
- The verify image repository coordinate should be standardized as `cockroach-migrate-verify` to match the existing verify-image naming pattern already used in test harnesses.
- The task explicitly requires trusted `master` pushes, and the current local and remote default branch is `master`.
- Story-21 task 03 exists for deeper hosted-CI debugging, but task 01 still requires real authenticated run/log evidence before completion.
  - Execution for task 01 should therefore include at least one honest hosted verification slice using the authenticated GitHub API wrapper.
  - If hosted failures reveal a materially different workflow shape than planned here, switch back to `TO BE VERIFIED`.
- `make test-long` is not a default end-of-task gate here.
  - Only run it if execution changes the ultra-long lane itself or changes the long-lane selection boundary.

## Current State Summary

- `.github/workflows/master-image.yml` currently publishes only two images:
  - runner from `./Dockerfile`
  - setup-sql from `./crates/setup-sql/Dockerfile`
- The current workflow is `push`-to-`master` only, with `publish` gated by `refs/heads/master`.
- The current workflow depends on several third-party convenience actions:
  - `dtolnay/rust-toolchain`
  - `docker/setup-buildx-action`
  - `docker/login-action`
  - `docker/build-push-action`
  - `aquasecurity/setup-trivy`
- The existing workflow-contract helper is tightly coupled to that old shape:
  - `load_master_image()`
  - `.github/workflows/master-image.yml`
  - runner + setup-sql only
  - `master`-specific trigger and README copy
- `PublishedImageContract` currently knows only runner and setup-sql, so the three-image published contract has no single owner yet.
- The repo already has strong per-image local contracts:
  - runner image contract
  - setup-sql image contract
  - verify image contract
- The novice-user README already treats published registry images as the supported path, but the workflow contract is still behind that product reality.

## Interface And Boundary Decisions

- Replace the old workflow identity with one workflow boundary that matches the product:
  - preferred file name: `.github/workflows/publish-images.yml`
  - preferred support loader: `GithubWorkflowContract::load_publish_images()`
- Keep `crates/runner/tests/ci_contract.rs` thin and behavior-focused.
  - It should describe public workflow behavior, not parse YAML directly.
- Extend `GithubWorkflowContract` into the single honest owner for workflow assertions about:
  - trusted triggers
  - concurrency
  - validation-before-publish ordering
  - three-image target set
  - multi-arch publish settings
  - manual dependency installation / direct shell usage where practical
  - tag rules
  - secret/perms containment
  - downstream publish-manifest outputs used by later registry-only verification
- Flatten published-image coordinates into one repo-owned boundary.
  - `PublishedImageContract` should stop being a two-string helper.
  - Preferred shape: one canonical list/spec owner for runner, setup-sql, and verify repositories.
  - `GithubWorkflowContract` and README contract helpers should consume that shared boundary instead of each inventing their own image list.
- Prefer one publish-target matrix or one small typed manifest seam over three hand-copied build/push step clusters.
  - Good:
    - one target list with image id, dockerfile, context, and repository name
  - Bad:
    - three separate piles of near-duplicate YAML plus three separate test code paths
- Keep official GitHub actions only where the platform itself is the owner and there is no cleaner shell replacement with equal clarity.
  - `actions/checkout` remains acceptable.
  - `actions/upload-artifact` remains acceptable if report artifacts are still needed.
  - Docker login/buildx/qemu/trivy/rust installation should move to explicit shell steps if practical on hosted runners.
- Release creation must remain manual and owner-controlled.
  - The publish workflow must not trigger on tags or releases and must not auto-create releases.

## Public Contract To Establish

- One fast contract fails if the workflow does not trigger only on `push` to `master`.
- One fast contract fails if the workflow allows PRs, `pull_request_target`, tags, issues, `workflow_dispatch`, `workflow_call`, `workflow_run`, or scheduled runs into the publish path.
- One fast contract fails if concurrency does not cancel an older in-progress trusted publish run when a newer `master` push lands.
- One fast contract fails if the workflow does not validate before any image publish work begins.
- One fast contract fails if the workflow target set is not exactly:
  - runner
  - setup-sql
  - verify
- One fast contract fails if the publish path is not multi-arch for both `linux/amd64` and `linux/arm64`.
- One fast contract fails if the publish path still depends on convenience publish actions instead of direct shell install/login/build usage where practical.
- One fast contract fails if any image is tagged with anything other than the full pushed commit SHA.
- One fast contract fails if `latest`, semver tags, branch tags, tag-derived refs, or release-derived refs are introduced into the automated publish path.
- One fast contract fails if publish-capable permissions or credentials are available outside the trusted `master` push path.
- One fast contract fails if the workflow does not emit one honest downstream manifest/output boundary for the published image refs so later registry-only verification can consume published artifacts instead of local builds.
- One fast contract fails if the README safety note or contract documentation still claims `master` or otherwise drifts from the enforced workflow behavior.

## Improve-Code-Boundaries Focus

- Primary smell:
  - workflow/product identity is split across the wrong boundaries:
    - workflow file name and loader are `master`-specific
    - image coordinates live partly in YAML env, partly in test helpers, and only for two images
    - the workflow contract currently hard-codes old runner/setup-sql assumptions
- Required cleanup during execution:
  - rename the workflow boundary away from `master-image`
  - make one support owner for workflow behavior and one support owner for published image coordinates
  - remove two-image drift by giving the repo one canonical three-image manifest boundary
  - prefer matrix-driven YAML plus typed support accessors over repeated per-image YAML string checks
- Smells to avoid:
  - adding a second workflow helper just for multi-arch or secret logic
  - scattering `refs/heads/master`, `ghcr.io`, or repository names across unrelated files
  - keeping `master-image.yml` and adding a second workflow beside it
  - baking verify image coordinates into ad hoc test strings without lifting them into `PublishedImageContract`

## Files And Structure To Add Or Change

- [x] `.github/workflows/publish-images.yml`
  - new canonical three-image workflow
  - trusted `push` to `master` only
  - validation before publish
  - concurrency cancel-on-newer-master-push
  - multi-arch publish for runner, setup-sql, verify
  - direct shell installation/login/build/publish where practical
  - downstream publish-manifest outputs for later registry-only consumers
- [x] `.github/workflows/master-image.yml`
  - delete after the new workflow contract is in place so the repo has one honest publish path
- [x] `crates/runner/tests/ci_contract.rs`
  - replace branch-drifted/two-image expectations with behavior-focused three-image `master`-only publish contracts
- [x] `crates/runner/tests/support/github_workflow_contract.rs`
  - rename loader and add typed helpers/assertions for:
    - trusted trigger set
    - forbidden trigger set
    - concurrency
    - target manifest
    - multi-arch build rules
    - direct shell install/login/build steps
    - publish outputs / manifest boundary
    - `master`-only trust gating
    - secret containment and redaction-related invariants
- [x] `crates/runner/tests/support/published_image_contract.rs`
  - expand into the canonical three-image coordinate owner
- [x] `crates/runner/tests/support/readme_published_image_contract.rs`
  - update shared published-image assertions if README still documents workflow/publish safety or image coordinates through the old two-image boundary
- [x] `README.md`
  - keep the CI publish-safety section aligned to `master`
  - mention the three-image publish boundary only if needed for truthful public documentation
- [x] No product runtime interfaces are expected to change
  - this task is workflow/test/doc contract work only

## TDD Execution Order

### Slice 1: Tracer Bullet For The New Workflow Identity

- [x] RED: add one failing contract that requires the canonical workflow boundary to be `publish-images.yml` and to trigger only on trusted `push` events to `master`
- [x] GREEN: add the smallest new workflow file and support-loader change needed to satisfy that contract
- [x] REFACTOR: delete the old `master-image` identity from support/test code instead of carrying both names forward

### Slice 2: Flatten The Three-Image Published Coordinate Boundary

- [x] RED: add the next failing contract that requires the published image target set to include runner, setup-sql, and verify from one shared boundary
- [x] GREEN: expand `PublishedImageContract` into one canonical three-image owner and wire the workflow contract helpers to consume it
- [x] REFACTOR: remove old two-image literals from `GithubWorkflowContract`, README helpers, and any other contract surface touched by this slice

### Slice 3: Prove Trusted Trigger Scope, Permissions, And Concurrency

- [x] RED: add failing workflow contracts for:
  - `push` to `master` only
  - explicit publish-job `if:` gate for trusted `master` pushes
  - no outsider-controlled triggers
  - no tag/release path
  - least-privilege permissions
  - cancel-in-progress concurrency for newer `master` pushes
- [x] GREEN: implement the minimum workflow guardrails to satisfy those contracts
- [x] REFACTOR: keep trust, permission, and concurrency accessors inside `GithubWorkflowContract`

### Slice 4: Prove Validation Happens Before Publish And Uses Direct Shell Steps

- [x] RED: add failing contracts that require the workflow to:
  - run repository validation before publish
  - install toolchain/dependencies directly where practical
  - avoid Docker convenience publish actions in favor of explicit shell steps
- [x] GREEN: move workflow steps to direct shell install/login/buildx/trivy usage as needed
- [x] REFACTOR: keep shell-step intent assertions stable and behavior-level, not brittle whole-script snapshots

### Slice 5: Prove Multi-Arch Three-Image Publish Rules

- [x] RED: add failing contracts that require:
  - exactly three publish targets
  - runner/context/dockerfile match the root Rust runtime image
  - setup-sql/context/dockerfile match the one-time SQL emitter image
  - verify/context/dockerfile match the verify-only Go slice image
  - `linux/amd64` and `linux/arm64` publication for every target
  - full-commit-SHA tags only
  - no `latest`
- [x] GREEN: implement the smallest truthful workflow target manifest and buildx publish commands to satisfy those rules
- [x] REFACTOR: prefer one target manifest or matrix seam rather than three hand-maintained step sets

### Slice 6: Prove Downstream Registry-Only Consumption Uses Published Outputs

- [x] RED: add a failing contract that requires the workflow to expose one manifest/output boundary for the three published image refs so later downstream verification can consume registry-published artifacts instead of rebuilding locally
- [x] GREEN: add the minimum workflow output/artifact step needed to surface those exact published refs
- [x] REFACTOR: keep the manifest shape owned by the workflow contract helper rather than string-searching raw YAML in unrelated tests

### Slice 7: Real Hosted Verification Through Authenticated Workflow API Access

- [x] RED: after local contract and repo lanes are green, use the authenticated GitHub API curl wrapper to inspect the first hosted workflow run for this path and treat any hosted failure as real RED
- [x] GREEN: iterate on workflow fixes until the hosted three-image publish run succeeds for the trusted `master` push path, with real confirmation of:
  - runner image build/publish
  - setup-sql image build/publish
  - verify image build/publish
  - `amd64` and `arm64` behavior
  - secret-gating expectations
  - log redaction not leaking credentials
- [x] REFACTOR: if hosted failures expose a wrong workflow shape, switch the plan back to `TO BE VERIFIED` instead of patching around the evidence

### Slice 8: Final Documentation And Repo Lanes

- [x] RED: if README or contract docs drift from the enforced workflow behavior, add the smallest failing doc expectation needed
- [x] GREEN: make only the truthful documentation changes needed for the enforced workflow contract
- [x] REFACTOR: run one final `improve-code-boundaries` pass so:
  - workflow behavior has one honest support owner
  - image coordinates have one honest support owner
  - the repo no longer carries `master`/two-image drift

## TDD Guardrails For Execution

- One failing test or hosted evidence slice at a time.
- Do not update workflow YAML first and retrofit tests afterward.
- Do not keep `master-image.yml` around as a second publish path.
- Do not satisfy the task with local-only success. Hosted GitHub run/log evidence is part of done.
- Do not use `docker/login-action`, `docker/build-push-action`, or similar convenience publish actions if a direct shell path is practical on the hosted runner.
- Do not allow any PR, fork, issue, schedule, tag, manual dispatch, or reusable-workflow path to reach publish secrets.
- Do not publish before validation succeeds.
- Do not publish `latest`.
- Do not swallow workflow parse failures, hosted-run failures, missing-step failures, login failures, build failures, publish failures, or log-redaction failures.
- If the required `master`-push hosted verification cannot be exercised honestly from the repo state reached during execution, switch back to `TO BE VERIFIED` immediately.

## Boundary Review Checklist

- [x] One honest workflow contract support boundary owns publish-workflow assertions
- [x] One honest published-image boundary owns runner/setup-sql/verify coordinates
- [x] The repo has one canonical three-image publish workflow, not an old `master-image` leftover plus a new one
- [x] `master` is used consistently across workflow, tests, and documentation touched by this task
- [x] Multi-arch and three-image target rules are enforced through repo-owned tests
- [x] Downstream registry-only consumers can use published refs surfaced by the workflow itself
- [x] No error path is swallowed

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` only if execution changes the ultra-long lane itself or its selection boundary
- [x] One final `improve-code-boundaries` pass after all required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after the required lanes and hosted verification both pass

Plan path: `.ralph/tasks/story-21-github-workflows-image-publish/01-task-fix-github-workflows-to-build-test-and-publish-the-three-image-split_plans/2026-04-19-three-image-master-publish-workflow-plan.md`

NOW EXECUTE
