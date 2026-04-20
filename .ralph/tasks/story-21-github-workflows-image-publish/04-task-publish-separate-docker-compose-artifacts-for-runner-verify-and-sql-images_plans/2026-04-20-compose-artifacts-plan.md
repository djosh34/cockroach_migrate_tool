# Plan: Publish Separate Docker Compose Artifacts For Runner, Verify, And Setup-SQL Images

## References

- Task:
  - `.ralph/tasks/story-21-github-workflows-image-publish/04-task-publish-separate-docker-compose-artifacts-for-runner-verify-and-sql-images.md`
- Current publish workflow and workflow contracts:
  - `.github/workflows/publish-images.yml`
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/support/github_workflow_contract.rs`
- Current published-image and README contracts:
  - `crates/runner/tests/support/published_image_contract.rs`
  - `crates/runner/tests/support/readme_published_image_contract.rs`
  - `crates/runner/tests/readme_contract.rs`
  - `crates/setup-sql/tests/image_contract.rs`
- Existing image artifact ownership:
  - `crates/runner/tests/support/image_build_target_contract.rs`
  - `crates/runner/tests/support/runner_image_artifact_harness.rs`
  - `crates/runner/tests/support/verify_image_artifact_harness.rs`
  - `crates/setup-sql/tests/support/source_bootstrap_image_harness.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the interface direction and test priorities in this turn.
- This turn is planning-only because task 04 had no plan file and no execution marker.
- The current public operator path is image-first and README-driven:
  - `setup-sql` is documented through `docker run`
  - `runner` is documented through `docker run`
  - `verify` is published in CI but not yet surfaced as a first-class operator contract in README
- The task requires three separate Compose contracts, not one combined operator stack:
  - one for `runner`
  - one for `verify`
  - one for `setup-sql`
- The operator path must work from published images and published artifacts, not from a repository checkout.
- Modern Compose config-style mounting is part of the public contract where it fits:
  - `runner` should use Compose `configs` for the mounted runtime config
  - `setup-sql` should use Compose `configs` for its mounted YAML inputs
  - `verify` should use a dedicated Compose contract instead of piggybacking on the runner examples
- If execution reveals that Compose `configs` do not fit one of the runtimes honestly without unsafe bind-mount hacks, switch this plan back to `TO BE VERIFIED`.

## Current State Summary

- `.github/workflows/publish-images.yml` publishes three container images and emits one `published-images.json` artifact, but no Compose artifacts.
- `README.md` gives novice-friendly published-image examples for:
  - `setup-sql`
  - `runner`
  It does not yet expose dedicated Compose contracts for any runtime.
- `crates/runner/tests/support/published_image_contract.rs` owns registry/repository identity for the three published images.
- `crates/runner/tests/support/image_build_target_contract.rs` separately owns workflow/build metadata for the same images.
- Adding Compose artifacts directly on top of the current structure would create a third parallel registry of image/runtime metadata.
- Existing tests already treat README text and workflow topology as public contracts, so Compose work should extend those surfaces rather than introducing hidden generator behavior with no coverage.

## Interface And Boundary Decisions

- Keep one canonical workflow:
  - `.github/workflows/publish-images.yml`
- Add one canonical published-artifact contract owner for operator-facing image metadata.
  - Preferred direction:
    - replace or absorb `PublishedImageContract` and the operator-facing parts of `ImageBuildTargetContract` into one shared contract that can answer:
      - image id
      - image repository
      - README env var name
      - compose artifact filename
      - compose service name
      - whether the runtime expects config-style YAML input
- Do not create:
  - a third ad hoc compose-target registry
  - stringly duplicated image metadata in README tests, workflow tests, and compose tests
- Keep workflow/build-only metadata separate if needed, but compose/publication naming must come from the same public contract as README image coordinates.
- Publish three standalone Compose files as artifacts alongside the existing image manifest.
  - Each Compose file must be usable on its own.
  - No combined all-in-one mandatory Compose contract.
- Keep the README examples copyable in-line.
  - The novice operator should be able to copy the Compose YAML and command examples directly from README without fetching repo files.
- Prefer generated publication artifacts from committed templates/spec data over hand-maintained shell heredocs in the workflow.
  - The workflow should publish already-reviewed Compose definitions, not improvise them in bash.

## Public Contract To Establish

- One contract fails if the repository no longer defines exactly three dedicated Compose artifacts:
  - runner
  - verify
  - setup-sql
- One contract fails if a Compose artifact points at a local build context instead of a published image reference.
- One contract fails if a Compose artifact stops using modern Compose config-style inputs where the runtime contract expects mounted YAML.
- One contract fails if the `runner` Compose contract stops exposing only the runtime command path.
- One contract fails if the `setup-sql` Compose contract stops exposing the one-time SQL emission path.
- One contract fails if the `verify` Compose contract is missing or collapses back into contributor-only or repo-clone guidance.
- One contract fails if the publish workflow stops shipping the Compose artifacts alongside the image-publication flow.
- One contract fails if README examples stop being copyable without a repository checkout.
- Existing contracts for:
  - trusted `push` to `master`
  - least-privilege publish permissions
  - canonical three-image publish
  - README published-image coordinates
  must remain green.

## Improve-Code-Boundaries Focus

- Primary smell:
  - operator-facing image identity is already duplicated across `PublishedImageContract` and `ImageBuildTargetContract`
  - task 04 would become muddier if Compose publication added a third copy of the same data
- Required cleanup during execution:
  - flatten the public image/runtime metadata into one honest contract owner for README and compose publication
  - keep workflow-only build concerns behind a smaller workflow-target boundary
  - avoid bash-generated YAML that hides the real operator contract inside workflow scripts
- Preferred cleanup shape:
  - one public spec module for published runtime artifacts
  - one workflow helper that reads that spec for publication assertions
  - README and Compose contract tests reusing that spec instead of hard-coded image names in multiple places
- Smells to avoid:
  - checking in three unrelated Compose files with no shared contract tests
  - inventing separate compose names in workflow YAML, README text, and tests
  - embedding operator YAML directly inside GitHub Actions shell without source-controlled templates/specs

## Files And Structure To Add Or Change

- [x] Add a new plans/source-controlled Compose artifact area
  - likely under a repo-owned directory such as `artifacts/compose/` or similar
  - store the canonical per-runtime Compose definitions there so workflow publication is deterministic
- [x] Add or refactor one shared published runtime artifact contract
  - likely replacing or subsuming:
    - `crates/runner/tests/support/published_image_contract.rs`
    - parts of `crates/runner/tests/support/image_build_target_contract.rs`
- [x] `.github/workflows/publish-images.yml`
  - publish the three Compose artifacts alongside the image manifest artifact
  - keep least-privilege and trusted-master-push rules intact
- [x] `crates/runner/tests/support/github_workflow_contract.rs`
  - assert Compose artifact publication and artifact names
- [x] `crates/runner/tests/ci_contract.rs`
  - add behavior-level contract coverage for the Compose publication flow
- [x] `crates/runner/tests/support/readme_published_image_contract.rs`
  - extend README/public contract coverage to include Compose examples
- [x] `crates/runner/tests/readme_contract.rs`
  - add README contract slices for dedicated runner, verify, and setup-sql Compose examples
- [x] Add new Compose-focused contract helpers/tests as needed
  - keep them behavior-driven and public-surface-focused
- [x] `README.md`
  - document copyable Compose examples for each runtime without repo checkout

## TDD Execution Order

### Slice 1: Tracer Bullet For Published Compose Artifact Identity

- [x] RED: add one failing contract that requires exactly three dedicated Compose artifact definitions owned by one public runtime spec instead of ad hoc strings
- [x] GREEN: introduce the smallest honest spec/template structure that can represent:
  - runner
  - verify
  - setup-sql
- [x] REFACTOR: remove or collapse duplicated public image metadata so Compose publication does not add a third registry

### Slice 2: Runner Compose Contract

- [x] RED: add one failing contract that requires a standalone runner Compose artifact using the published runner image and config-style mounted runner YAML
- [x] GREEN: add the runner Compose definition and wire README/public contract coverage to it
- [x] REFACTOR: keep runner Compose assertions in a dedicated public contract helper, not scattered through YAML string checks

### Slice 3: Setup-SQL Compose Contract

- [x] RED: add one failing contract that requires a standalone setup-sql Compose artifact using the published setup-sql image and config-style mounted setup YAML for SQL emission
- [x] GREEN: add the setup-sql Compose definition and README/example coverage
- [x] REFACTOR: keep the one-time SQL-emitter path clearly separate from the runtime path

### Slice 4: Verify Compose Contract

- [x] RED: add one failing contract that requires a standalone verify Compose artifact using the published verify image
- [x] GREEN: add the verify Compose definition and surface it in README as a first-class operator contract
- [x] REFACTOR: keep verify isolated instead of folding it into runner documentation or contributor-only flows

### Slice 5: Workflow Publication Of Compose Artifacts

- [x] RED: add one failing workflow contract that requires the publish workflow to ship the three Compose files alongside the published image flow
- [x] GREEN: update `.github/workflows/publish-images.yml` to upload the Compose artifacts through a deterministic source-controlled boundary
- [x] REFACTOR: keep publication names and paths derived from the shared runtime spec

### Slice 6: README Copyability And Final Boundary Cleanup

- [x] RED: add failing README contract coverage that requires copyable Compose examples for runner, verify, and setup-sql without repo checkout or local builds
- [x] GREEN: update `README.md` so each runtime has a dedicated Compose contract example that references only published images and operator-managed config files
- [x] REFACTOR: run one final `improve-code-boundaries` pass so public runtime metadata is defined once and reused honestly across README, workflow, and tests

## TDD Guardrails For Execution

- One failing test slice at a time.
- Do not bulk-add all Compose files before the first failing contract exists.
- Do not document Compose examples first and retrofit artifact publication later.
- Do not publish Compose YAML from bash heredocs if the same contract can live as a source-controlled artifact/template.
- Do not let verify remain an implicit or contributor-only path.
- Do not duplicate image/runtime metadata across three helpers after the refactor.
- If the Compose spec boundary starts growing into two overlapping registries again, stop and switch back to `TO BE VERIFIED`.

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] Do not run `make test-long` unless execution proves this task changed ultra-long selection or the story is being finished
- [x] One final `improve-code-boundaries` pass after the required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-21-github-workflows-image-publish/04-task-publish-separate-docker-compose-artifacts-for-runner-verify-and-sql-images_plans/2026-04-20-compose-artifacts-plan.md`

NOW EXECUTE
