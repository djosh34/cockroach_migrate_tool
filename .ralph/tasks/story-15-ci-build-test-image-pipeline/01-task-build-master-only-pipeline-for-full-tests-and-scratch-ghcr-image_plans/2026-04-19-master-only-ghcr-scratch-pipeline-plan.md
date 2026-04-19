# Plan: Master-Only Full-Test GHCR Scratch Image Pipeline

## References

- Task: `.ralph/tasks/story-15-ci-build-test-image-pipeline/01-task-build-master-only-pipeline-for-full-tests-and-scratch-ghcr-image.md`
- Existing repository quality lanes:
  - `Makefile`
- Existing container contract and long-lane coverage:
  - `Dockerfile`
  - `crates/runner/tests/long_lane.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
  - `crates/runner/tests/support/runner_container_process.rs`
- Existing public Docker documentation:
  - `README.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public interface and behavior priorities in this turn.
- This turn is planning-only because the task file had no execution marker and the instructions require a separate plan before implementation.
- The repository currently has no `.github/workflows` pipeline, so the first tracer bullet should assert the missing workflow through a repository-owned test instead of adding YAML first and calling it done.
- The published image contract must stay aligned with the already-tested public container contract rather than inventing a second image shape just for CI.
- A true scratch runtime image means the final image stage is `FROM scratch` and contains only the `runner` binary. If the first RED slice proves the binary cannot satisfy the current runtime contract when built for scratch without changing public behavior, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.
- If the first RED slice proves the workflow contract cannot be asserted cleanly from repo-owned tests without inventing a brittle ad hoc YAML mini-framework, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - the workflow triggers only on pushes to `master`
  - the workflow never runs on pull requests
  - the CI path runs the full repository validation suite instead of a reduced subset
  - the published image is tagged from the pushed commit identity and is pushed only to GHCR
  - the workflow never publishes `latest` or automatic version tags
  - the runtime image is truly `scratch` and contains only the compiled binary
- Next-priority behavior to prove:
  - workflow structure keeps registry-specific coordinates isolated so Quay can be added later without rewiring the whole job
- Lower-priority concerns:
  - CI caching polish
  - release-notes or deployment automation
  - pull-request workflows

## Problem To Fix

- There is currently no GitHub Actions workflow at all, so the repository cannot prove a guarded `master`-only publish path.
- The current `Dockerfile` builds a release binary and ships it in `debian:bookworm-slim`, which violates the task's required scratch-only runtime contract.
- The existing container contract support proves direct-entrypoint behavior, but image-build and runtime-launch knowledge is still split across multiple support files. That makes it too easy for the workflow, Dockerfile, and long-lane tests to drift into slightly different image contracts.
- There is no repository-owned test that guards workflow trigger scope, validation-lane coverage, or publish-tag rules. Without that, future workflow edits could silently widen execution or publish unsafe tags.

## Interface And Boundary Decisions

- Keep the public runtime CLI unchanged. This task is about CI and image packaging, not about changing the `runner` command surface.
- Introduce one repository-owned workflow contract test boundary that reads the checked-in workflow YAML and asserts:
  - trigger scope
  - required validation commands
  - registry and tag behavior
  - no PR trigger path
- Keep image-contract assertions owned by one support boundary instead of duplicating Dockerfile and image-inspect expectations across unrelated tests.
  - preferred owner: extend `crates/runner/tests/support/runner_docker_contract.rs`
- Keep registry-specific values localized in the workflow through one small image-coordinate boundary, such as one metadata step or one env block, instead of scattering `ghcr.io` strings and tag formats across multiple steps.
- Prefer fast contract tests for workflow and Dockerfile shape, plus one existing ignored Docker long lane for executable image behavior. Do not rely on manual YAML inspection.

## Public Contract To Establish

- One repository-owned fast contract fails if the workflow stops being `push`-to-`master` only.
- One fast contract fails if `pull_request`, tag pushes, manual release tagging, or `latest` publishing is introduced.
- One fast contract fails if the workflow stops running the full validation suite required by the repo:
  - `make check`
  - `make test`
  - `make test-long`
- One fast contract fails if the published image reference is not commit-derived and GHCR-scoped.
- One fast contract fails if the workflow shape loses its clean extension point for adding another registry later.
- One fast or ignored executable contract fails if the runtime image stops being `scratch`, stops using the `runner` binary directly, or starts shipping extra runtime payload.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - the image contract currently lives in too many places:
    - `Dockerfile`
    - `runner_docker_contract.rs`
    - `runner_image_harness.rs`
    - `runner_container_process.rs`
- Required cleanup during execution:
  - centralize image-build and runtime-image assertions behind one honest test-support owner
  - extend that owner to cover the scratch-image contract instead of creating a second helper with overlapping Dockerfile knowledge
  - keep workflow assertions in a dedicated workflow-contract support file rather than sprinkling raw YAML string searches through test files
- If `runner_image_harness.rs` and `runner_container_process.rs` end up differing only by thin wrappers after the extraction, merge or delete the thinner boundary.
- Do not add another fake abstraction layer. The support boundary must own real contract knowledge, not just forward literals.

## Files And Structure To Add Or Change

- [x] `.github/workflows/master-image.yml`
  - new workflow for `push` on `master` only, full validation, scratch-image publish to GHCR
- [x] `Dockerfile`
  - change the final runtime stage from Debian to `scratch` and make the final payload only the compiled `runner` binary
- [x] `crates/runner/tests/ci_contract.rs`
  - new fast workflow contract tests for trigger scope, validation commands, tag rules, and registry boundary shape
- [x] `crates/runner/tests/support/github_workflow_contract.rs`
  - new support owner for loading and asserting workflow structure without ad hoc test-local string parsing
- [x] `crates/runner/tests/support/runner_docker_contract.rs`
  - extend the existing image contract owner to cover scratch-runtime assertions and canonical image build expectations
- [x] `crates/runner/tests/support/runner_image_harness.rs`
  - likely reuse or simplify around the expanded image-contract boundary
- [x] `crates/runner/tests/support/runner_container_process.rs`
  - simplify or merge only if execution proves command-shape ownership still overlaps after the extraction
- [x] `crates/runner/tests/long_lane.rs`
  - strengthen the ignored long lane with scratch-image contract checks if they are not already covered by the support boundary
- [x] `README.md`
  - only if the public Docker quick start needs a small update so the documented image contract stays truthful after the scratch migration
- [x] No product CLI interface changes are expected
  - if RED exposes a real runtime incompatibility from the scratch move, fix the real image/runtime contract rather than weakening the test

## TDD Execution Order

### Slice 1: Tracer Bullet For Workflow Trigger Scope

- [x] RED: add one failing repository-owned test that requires a checked-in workflow file and asserts the workflow triggers only on `push` to `master`
- [x] GREEN: add the smallest workflow and support code needed to make that trigger contract pass
- [x] REFACTOR: move raw workflow loading/parsing behind a dedicated support boundary instead of leaving YAML shape checks in the test file

### Slice 2: Prove The Workflow Runs The Full Validation Suite

- [x] RED: add the next failing contract that requires the workflow to run the full repository validation lane rather than a smoke subset
- [x] GREEN: make the workflow run the minimal required set:
  - `make check`
  - `make test`
  - `make test-long`
- [x] REFACTOR: keep command-shape assertions centralized so the workflow contract does not duplicate shell fragments across tests

### Slice 3: Prove Publish Rules, Tagging, And GHCR Scope

- [x] RED: add a failing contract for the first missing publish rule:
  - GHCR push exists
  - tag derives from the pushed commit identity
  - no `latest`
  - no automatic version tags
  - no PR publish path
- [x] GREEN: implement the smallest workflow changes needed to satisfy the publish contract
- [x] REFACTOR: isolate registry coordinates and tag composition so a future Quay publish can reuse the same contract shape instead of editing multiple unrelated steps

### Slice 4: Prove The Runtime Image Is A Real Scratch Binary-Only Artifact

- [x] RED: add a failing contract for the current Debian runtime image and the current non-scratch final stage
- [x] GREEN: change the Docker build to produce a scratch final image containing only the `runner` binary
- [x] REFACTOR: extend the existing image-contract support boundary to own scratch assertions instead of duplicating Dockerfile and inspect logic in multiple test files

### Slice 5: Prove The Existing Executable Image Path Still Works

- [x] RED: strengthen the ignored Docker long lane so it fails if the built image stops satisfying the public runtime contract after the scratch migration
- [x] GREEN: make the minimum build or runtime adjustments needed for the long lane to pass with the scratch image
- [x] REFACTOR: if `runner_image_harness.rs` and `runner_container_process.rs` overlap after the image-contract extraction, merge or flatten the thinner one

### Slice 6: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so workflow assertions and image assertions each have one honest owner with no drift

## TDD Guardrails For Execution

- Every new test or assertion must fail before the supporting workflow, Dockerfile, or test-support change is added.
- Do not satisfy this task by adding a workflow and skipping repository-owned tests for its trigger and publish rules.
- Do not satisfy the scratch requirement by keeping Debian or Alpine in the final runtime stage.
- Do not publish `latest`, semantic version tags, or branch tags from this task.
- Do not add a pull-request workflow as a side effect of making GitHub Actions work.
- Do not swallow Docker build, workflow-contract parse, login, or publish errors. Any such failure is task-relevant and must fail loudly.
- Do not broaden this task into Quay publishing. Only leave a clean extension seam.

## Boundary Review Checklist

- [x] One honest support boundary owns workflow-structure assertions
- [x] One honest support boundary owns scratch-image contract assertions
- [x] Registry coordinates are not scattered through unrelated workflow steps
- [x] Dockerfile, workflow, and long-lane tests describe the same shipped image contract
- [x] No PR trigger or publish path can hide behind untested YAML drift
- [x] No error path is swallowed

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
