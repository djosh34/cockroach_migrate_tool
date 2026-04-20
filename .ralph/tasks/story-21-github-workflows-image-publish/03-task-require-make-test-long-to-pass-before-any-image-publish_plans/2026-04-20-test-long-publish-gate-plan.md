# Plan: Require `make test-long` Before Image Publish

## References

- Task:
  - `.ralph/tasks/story-21-github-workflows-image-publish/03-task-require-make-test-long-to-pass-before-any-image-publish.md`
- Current workflow and workflow contracts:
  - `.github/workflows/publish-images.yml`
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/github_workflow_contract.rs`
  - `crates/runner/tests/support/image_build_target_contract.rs`
- Repo validation lanes:
  - `Makefile`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the interface and behavior priorities in this turn.
- This turn is planning-only because the task had no existing plan path or execution marker.
- The current publish workflow already has the desired image/publication topology:
  - canonical three-image set
  - native `linux/amd64` and `linux/arm64` publish lanes
  - manifest fan-in after publish
- The missing safety boundary is entirely pre-publish validation:
  - `publish-image` depends only on `validate`
  - `validate` runs `make check` and `make test`
  - `make test-long` is currently forbidden by the workflow contract
- `make lint` is an alias of `make check` in `Makefile`, but the repository policy still requires both commands to pass before the task may be complete.
- The execution turn must preserve the trusted-trigger and least-privilege publish path while making the long lane an explicit prerequisite.
- If the chosen validation topology still leaves the long lane implicit or easy to bypass in the workflow graph, execution must switch this plan back to `TO BE VERIFIED`.

## Current State Summary

- `.github/workflows/publish-images.yml` has one `validate` job, one `publish-image` matrix job, and one `publish-manifest` fan-in job.
- The `validate` job currently restores caches, installs Rust/PostgreSQL tooling, and runs:
  - `make check`
  - `make test`
- `publish-image` currently declares `needs: validate`, so publication is blocked only on the fast/default validation slice.
- `crates/runner/tests/ci_contract.rs` currently encodes the old boundary through:
  - `workflow.assert_runs_validation_commands(&["make check", "make test"]);`
- `crates/runner/tests/support/github_workflow_contract.rs` currently hard-codes the obsolete rule that the publish workflow must not run `make test-long`.
- Boundary smell from `improve-code-boundaries`:
  - pre-publish validation policy lives as an incidental shell script detail inside one mixed `validate` job
  - the workflow contract still treats `make test-long` as an anti-feature
  - reviewers cannot see a distinct long-lane gate in the publish dependency graph

## Interface And Boundary Decisions

- Keep one canonical workflow:
  - `.github/workflows/publish-images.yml`
- Keep `crates/runner/tests/ci_contract.rs` thin and behavior-focused.
  - It should describe the public CI contract, not parse YAML inline.
- Keep image identity in `ImageBuildTargetContract`.
  - This task is not about moving image metadata.
- Move pre-publish validation from one mixed shell boundary to one explicit workflow topology.
  - Preferred topology:
    - `validate-fast`
    - `validate-long`
    - `publish-image`
    - `publish-manifest`
- `validate-fast` owns the default repository lanes that must pass before publish:
  - `make check`
  - `make lint`
  - `make test`
- `validate-long` owns the ultra-long repository lane:
  - `make test-long`
- `publish-image` must depend on both validation jobs, not just the fast/default one.
- `publish-manifest` should continue to depend only on `publish-image`.
  - The publish gate should stay at the publication boundary rather than spreading validation logic into the manifest step.
- Keep validation-lane expectations behind one honest workflow-contract owner instead of scattering command and job-name literals across tests.
  - Preferred direction:
    - add a narrow validation-lane spec inside `GithubWorkflowContract` support
    - avoid creating a separate parallel registry that merely repeats YAML

## Public Contract To Establish

- One contract fails if the publish workflow no longer runs `make check` before publishing.
- One contract fails if the publish workflow no longer runs `make lint` before publishing.
- One contract fails if the publish workflow no longer runs `make test` before publishing.
- One contract fails if the publish workflow no longer runs `make test-long` before publishing.
- One contract fails if the long lane is hidden back inside a mixed shell step with no explicit pre-publish workflow dependency.
- One contract fails if `publish-image` no longer depends on both validation boundaries.
- One contract fails if `publish-manifest` is allowed to bypass the validation/publish ordering.
- One contract fails if the README still describes publish as gated only by the shorter/default validation path.
- Existing contracts for:
  - trusted `push` to `master`
  - least-privilege permissions
  - canonical three-image set
  - native platform publish
  - manifest fan-in
  must remain green.

## Improve-Code-Boundaries Focus

- Primary smell:
  - the workflow has honest owners for image identity and publish topology, but pre-publish validation is still a mixed responsibility hidden inside `validate`
- Required cleanup during execution:
  - remove the obsolete negative rule that `make test-long` must not run
  - replace it with a positive validation-lane contract that states what must pass before publish
  - make the long lane a first-class workflow dependency instead of an incidental shell line
- Smells to avoid:
  - keeping both the old `validate` job and new validation jobs alive at the same time
  - duplicating the same validation-lane names and commands across YAML, tests, and docs without one contract owner
  - inventing a large new helper hierarchy just to describe two validation jobs

## Files And Structure To Add Or Change

- [x] `.github/workflows/publish-images.yml`
  - replace the single mixed `validate` gate with explicit fast and long validation jobs
  - make `publish-image` depend on both validation jobs
  - preserve cache/toolchain setup honestly for both validation paths
- [x] `crates/runner/tests/ci_contract.rs`
  - replace the old validation assertion with behavior-level checks for the new pre-publish gate
- [x] `crates/runner/tests/support/github_workflow_contract.rs`
  - delete the obsolete `make test-long` prohibition
  - add assertions for explicit validation-lane topology and required commands
  - keep YAML parsing and workflow-boundary ownership here
- [x] `README.md`
  - update the CI publish safety section so reviewers can see that publish is blocked on the fast/default suite and the ultra-long lane
- [x] No product runtime interfaces should change
  - this task is workflow/test/doc contract work only

## TDD Execution Order

### Slice 1: Tracer Bullet For The Long-Lane Publish Gate

- [x] RED: add one failing contract that requires an explicit pre-publish long-lane dependency instead of the current single `validate` gate
- [x] GREEN: make the smallest truthful workflow change that introduces a distinct `validate-long` job running `make test-long` and makes `publish-image` depend on it
- [x] REFACTOR: keep the job-name and command assertions owned by `GithubWorkflowContract`, not duplicated in the test file

### Slice 2: Fast Validation Boundary Must Remain Explicit

- [x] RED: add one failing contract that requires the fast/default pre-publish boundary to keep running:
  - `make check`
  - `make lint`
  - `make test`
- [x] GREEN: split or rename the old `validate` job into an honest fast-validation boundary instead of leaving fast and long concerns mixed together
- [x] REFACTOR: use one narrow validation-lane spec inside the workflow helper so command expectations stay centralized

### Slice 3: Publish Ordering Must Fail Loudly If Future Edits Bypass The Full Suite

- [x] RED: add one failing contract that requires `publish-image` to wait on both validation jobs and `publish-manifest` to remain downstream of `publish-image`
- [x] GREEN: wire the workflow `needs` graph so publication cannot start after only the short/default suite
- [x] REFACTOR: remove the old validation ordering assertions instead of keeping both old and new topologies around

### Slice 4: Documentation Must Match The New Safety Model

- [x] RED: add or tighten the README contract so the CI publish safety section describes the long-lane gate truthfully
- [x] GREEN: update the README to explain that image publication is blocked on:
  - `make check`
  - `make lint`
  - `make test`
  - `make test-long`
- [x] REFACTOR: keep the README contract focused on behavior and trust boundaries, not exact shell wording

### Slice 5: Final Boundary Cleanup And Repo Lanes

- [x] RED: if any contract still permits the old fast-only publish path, add the smallest failing coverage needed
- [x] GREEN: complete the workflow/test/doc cleanup and update the task file only after the required repository lanes pass
- [x] REFACTOR: run one final `improve-code-boundaries` pass so pre-publish validation topology is explicit, honest, and not duplicated

## TDD Guardrails For Execution

- One failing test slice at a time.
- Do not bulk-edit the workflow first and retrofit tests afterward.
- Do not hide `make test-long` inside the old `validate` shell script and call that sufficient.
- Do not keep the old negative contract that forbids `make test-long`.
- Do not create fake success by documenting the long lane without wiring the actual `needs` graph.
- If the chosen validation topology still leaves the long gate ambiguous in GitHub Actions review UI, switch back to `TO BE VERIFIED`.

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required lane passes

## Execution Outcome

- Workflow boundary: `.github/workflows/publish-images.yml` now exposes explicit `validate-fast` and `validate-long` jobs, and `publish-image` is blocked on both before any registry publish starts.
- Contract boundary: `GithubWorkflowContract` now owns the fast/long validation topology, cache/install expectations, permission isolation, publish ordering, and README safety contract instead of scattering those assertions across test files.
- Repo proof: `make check`, `make lint`, `make test`, and `make test-long` all passed after execution.
- Additional boundary cleanup discovered by the required verification lanes:
  - fixed a Postgres contract-harness port-ownership race in `bootstrap_contract`, `reconcile_contract`, and `webhook_contract`
  - tightened `DefaultBootstrapHarness` so verify-image convergence is not treated as complete until tracking progress is fully reconciled
  - corrected `MultiMappingHarness` to audit the honest six-command bootstrap boundary for the two-mapping source bootstrap path

Plan path: `.ralph/tasks/story-21-github-workflows-image-publish/03-task-require-make-test-long-to-pass-before-any-image-publish_plans/2026-04-20-test-long-publish-gate-plan.md`

NOW EXECUTE
