# Plan: Ultra Simple Nix CI Workflow With Cached Parallel Image Artifacts

## References

- Task:
  - `.ralph/tasks/story-31-nix-github-workflow-cache-and-image-artifacts/01-task-create-ultra-simple-nix-github-workflow-with-cached-parallel-builds.md`
- Existing Nix and local quality entrypoints:
  - `flake.nix`
  - `Makefile`
- Existing published-image and local image-artifact contract surfaces:
  - `README.md`
  - `crates/runner/tests/support/nix_image_artifact_harness.rs`
  - `crates/runner/tests/support/runner_image_artifact_harness.rs`
  - `crates/runner/tests/support/verify_image_artifact_harness.rs`
- Skills required during execution:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn is planning-only because the task file had no plan path yet and no execution marker.
- The task markdown is treated as approval for the public workflow shape and the priority behaviors.
- This task is a real TDD exception.
  - It is workflow/infrastructure work.
  - Execution must not invent brittle repo-owned tests that assert YAML strings or shell fragments.
  - The red/green loop during execution must therefore use real command execution and hosted workflow evidence instead:
    - local `nix`, `docker`, and OCI-tool commands
    - hosted GitHub Actions logs for Magic Nix Cache reuse and matrix behavior
- The repository currently has no checked-in `.github/workflows/*.yml` file.
- The repository already exposes two published image families:
  - `runner`
  - `verify`
- The task phrase "one single multi-platform image tag" is interpreted per published image family.
  - Expected final user-facing tags are therefore:
    - `cockroach-migrate-runner:<git-sha>`
    - `cockroach-migrate-verify:<git-sha>`
  - This task does not collapse the two distinct images into one repository name.
- The current Nix image flow is only a local single-system Docker-archive path.
  - `flake.nix` exposes `runner-image` and `verify-image`
  - tests load those archives into Docker and retag the loaded `:nix` image locally
  - there is no current multi-platform artifact contract and no current GHCR-ready OCI artifact handoff
- Local verification on this machine can honestly prove the current-system path, retagging, and artifact shape.
- This host currently has `docker` and `nix`, but not `skopeo`, `oras`, `crane`, or `gh`.
  - Execution must therefore source any required OCI tooling from Nix rather than assuming host-global binaries.
  - Hosted GitHub Actions evidence will need to come from public workflow logs after push rather than from a local `gh` CLI.
- Hosted workflow logs must provide the proof for:
  - `aarch64` image generation
  - cross-run Magic Nix Cache reuse
  - parallel hosted job topology
- If execution proves that one single tagged multi-platform artifact cannot be produced honestly from the Nix-built images without a registry round-trip or hidden rebuild, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.
- If execution proves the task owner really intended to handle only one of the two published images instead of both `runner` and `verify`, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately rather than guessing.

## Approval And Verification Priorities

- Highest-priority behaviors to prove:
  - GitHub Actions contains one intentionally small Nix-owned workflow
  - the workflow enables Magic Nix Cache
  - test execution and image generation are separate jobs that can run in parallel
  - image generation uses repository Nix outputs directly rather than a Dockerfile rebuild path
  - the generated artifact is tagged with the exact git commit SHA after generation
  - each published image family is handed off as one user-facing multi-platform image tag for that commit SHA rather than separate per-arch user-facing tags
  - a later GHCR task can upload the generated artifact directly without rebuilding
- Next-priority behaviors to prove:
  - the workflow graph is small enough that the Nix commands are the source of truth rather than embedded shell logic
  - hosted runs show Magic Nix Cache reuse rather than rebuilding every dependency from scratch on repeated runs
- Lower-priority concerns:
  - README polish unless the current published-image story becomes false
  - release promotion or mutable tags
  - GHCR credentials or publish permissions

## Problem To Fix

- There is currently no GitHub Actions workflow at all for this repository.
- `flake.nix` contains a large `githubCiWorkflowCatalog`/matrix data model that is currently unused anywhere else in the repo.
  - That is the main boundary smell for this task.
  - It keeps GitHub workflow concepts in Nix without actually owning a workflow path.
- The current image artifact contract is too local and too narrow.
  - it builds one platform-local image archive
  - it loads that archive into Docker
  - it retags a local image name
  - it does not define an OCI artifact handoff for later registry upload
- The future GHCR upload task needs a direct artifact handoff, not a rebuild.
- The task explicitly forbids drifting back into Dockerfile-owned CI logic.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - CI/image-publish knowledge is split between:
    - dead GitHub workflow catalog data in `flake.nix`
    - ad hoc image package names in the current test harnesses
    - the future workflow file that does not exist yet
- Required cleanup during execution:
  - remove the dead `githubCiWorkflowCatalog` boundary from `flake.nix`
  - replace it with one smaller Nix-owned image-artifact boundary that the workflow can call directly
  - keep per-image metadata in one place
  - keep workflow YAML tiny so it wires jobs together rather than re-implementing build logic
- Smells to avoid:
  - reintroducing Dockerfile-based CI build logic
  - scattering image names, package attrs, and artifact paths across multiple workflow steps
  - keeping both the dead workflow catalog and a second new CI abstraction
  - adding fake test files that only parse YAML text

## Interface And Boundary Decisions

- Keep the existing public local quality commands unchanged:
  - `make check`
  - `make lint`
  - `make test`
- Introduce one Nix-owned platform artifact command for the workflow and for local verification.
  - Proposed public shape:
    - `nix run .#ci-platform-image-artifacts -- --git-sha <sha> --output-dir <dir>`
  - Behavior:
    - builds the current platform's `runner` and `verify` images from Nix
    - retags them to the exact commit SHA
    - exports OCI-form artifacts plus small machine-readable metadata for later assembly
    - writes deterministic output paths under the requested directory
- Introduce one Nix-owned artifact assembly command.
  - Proposed public shape:
    - `nix run .#ci-multi-platform-image-artifacts -- --git-sha <sha> --input-dir <downloaded-platform-artifacts-dir> --output-dir <dir>`
  - Behavior:
    - consumes the per-platform OCI artifacts and metadata
    - assembles one single multi-platform artifact per image family for the commit SHA
    - produces artifacts that a later GHCR task can upload directly without rebuilding
- Prefer OCI artifacts over Docker archive handoff for the final multi-platform output.
  - Docker archive is not an honest long-term boundary for a single multi-platform image tag.
  - The final artifact should therefore be one OCI layout or OCI archive per image family, with the exact final format chosen by whichever tool can express the multi-platform tag honestly without registry I/O.
- Keep the workflow itself intentionally small:
  - install/enable Nix
  - enable Magic Nix Cache
  - run `nix run .#check` and `nix run .#test` in the validation lane
  - run the platform artifact command in a platform matrix lane
  - run the assembly command in a final artifact lane
  - upload the assembled OCI artifact(s)
- Do not implement GHCR publishing in this task.
  - The final artifact path and later upload command must be documented in task notes instead.

## Expected Workflow Shape

- `validate`
  - one hosted Linux job
  - installs Nix
  - enables Magic Nix Cache
  - runs `nix run .#check`
  - runs `nix run .#test`
- `image-platform`
  - matrix over the two hosted platforms:
    - `ubuntu-24.04`
    - `ubuntu-24.04-arm`
  - installs Nix
  - enables Magic Nix Cache
  - runs `nix run .#ci-platform-image-artifacts -- --git-sha "$GITHUB_SHA" --output-dir "$RUNNER_TEMP/image-artifacts"`
  - uploads the platform artifact directory
- `image-assemble`
  - needs only the `image-platform` matrix jobs
  - downloads the platform artifact directories
  - runs `nix run .#ci-multi-platform-image-artifacts -- --git-sha "$GITHUB_SHA" --input-dir <downloaded-dir> --output-dir "$RUNNER_TEMP/final-image-artifacts"`
  - uploads the final assembled OCI artifact(s)
- Resulting topology:
  - `validate` and `image-platform` run in parallel
  - `image-assemble` runs after all platform artifact jobs finish
  - GHCR upload is deliberately absent

## Files Expected To Change During Execution

- [ ] `.github/workflows/nix-ci.yml`
  - new ultra-simple Nix-owned workflow
- [ ] `flake.nix`
  - remove dead GitHub CI catalog data
  - add the smaller image metadata boundary plus the Nix apps used by the workflow
- [ ] `README.md`
  - only if the published-image/operator story becomes false without a small truth-fixing note
- [ ] Potential repo script helpers under `scripts/ci/`
  - only if a small script is the clearest implementation of the Nix-owned artifact commands
  - the workflow must still call the Nix apps, not the scripts directly
- [ ] `.ralph/tasks/story-31-nix-github-workflow-cache-and-image-artifacts/01-task-create-ultra-simple-nix-github-workflow-with-cached-parallel-builds.md`
  - record the plan path now
  - mark acceptance and `<passes>true</passes>` only after execution and verification are complete

## Execution Order

### Slice 0: Confirm The Real Starting Failure Mode

- [ ] RED: prove the repo currently has no workflow and no direct Nix-owned multi-platform artifact handoff
- [ ] GREEN: document the exact missing seams to replace during execution
- [ ] REFACTOR: keep the design centered on one image-artifact boundary instead of extending the dead workflow catalog

### Slice 1: Replace The Dead CI Catalog With One Honest Nix Image Boundary

- [ ] RED: prove the current `flake.nix` GitHub workflow catalog is dead data and that the current image artifact path is only local Docker retagging
- [ ] GREEN: introduce the smallest Nix-owned image metadata and command surface needed for:
  - current-platform image export
  - commit-SHA retagging
  - later multi-platform assembly
- [ ] REFACTOR: delete the dead `githubCiWorkflowCatalog` and any now-redundant matrix metadata rather than layering a second abstraction on top

### Slice 2: Make Current-Platform Artifact Generation Real

- [ ] RED: run the new current-platform artifact command locally and let it fail until the output contract is honest
- [ ] GREEN: make it successfully produce:
  - platform-specific OCI-form artifacts for `runner` and `verify`
  - exact commit-SHA tagging
  - machine-readable metadata that the assembly command can consume
- [ ] REFACTOR: keep image names, package attrs, and artifact naming rules owned by one Nix boundary

### Slice 3: Make Multi-Platform Assembly Real

- [ ] RED: attempt to assemble the final artifact shape from platform metadata and let it fail until the resulting artifact is a truthful single-tag multi-platform handoff
- [ ] GREEN: implement the smallest honest assembly path that produces one final artifact per image family for the commit SHA
- [ ] REFACTOR: keep the assembly metadata minimal and delete any temporary per-step shape that is not part of the final boundary

### Slice 4: Add The Ultra-Simple Workflow

- [ ] RED: validate that the repo still has no workflow and that the new Nix commands are not yet wired into hosted jobs
- [ ] GREEN: add `.github/workflows/nix-ci.yml` with:
  - Nix installation
  - Magic Nix Cache
  - a validation job using `nix run .#check` and `nix run .#test`
  - a platform matrix image-generation job using the Nix platform-artifact command
  - a final assembly job using the Nix assembly command
  - artifact upload only, with no GHCR publish step
- [ ] REFACTOR: keep the YAML tiny and push command complexity back into the Nix apps instead of shell steps

### Slice 5: Local Verification And Evidence Capture

- [ ] RED: the task is not done until the current-system Nix image generation and retagging flow succeeds locally with real commands
- [ ] GREEN: run and record the exact local verification commands, including:
  - the Nix platform-artifact command
  - the exact resulting artifact path(s)
  - the OCI/Docker verification command(s) used to confirm the tag and artifact shape
- [ ] REFACTOR: if local verification reveals a muddier interface than planned, simplify the command surface before moving on

### Slice 6: Hosted Cache And Parallelism Verification

- [ ] RED: the task is not done until hosted GitHub Actions evidence shows the intended parallel topology and Magic Nix Cache reuse behavior
- [ ] GREEN: inspect the hosted run logs and record concrete evidence for:
  - `validate` running in parallel with `image-platform`
  - both hosted platforms producing platform artifacts
  - later `image-assemble` consuming those artifacts
  - Magic Nix Cache hits or reuse behavior on a repeated run
- [ ] REFACTOR: if hosted logs show unnecessary rebuilds or duplicated command logic, simplify the workflow or Nix boundary instead of explaining away the waste

## Execution Guardrails

- Do not add repo-owned tests that only assert strings in workflow YAML, Dockerfiles, or scripts.
- Do not run `make test-long` for this task.
- Do not add GHCR credentials, registry write permissions, or publish logic.
- Do not rebuild images outside Nix in the workflow.
- Do not keep separate user-facing per-architecture tags as the final artifact contract.
- Do not swallow any `nix`, `docker`, OCI-tool, or hosted-workflow errors.
- If execution needs exact current action names or versions for Nix installation or Magic Nix Cache, verify them from the official sources before editing.

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] Current-system local Nix image generation and retagging flow succeeds and is recorded in the task notes
- [ ] Hosted GitHub Actions evidence for Magic Nix Cache reuse and parallel job topology is recorded in the task notes
- [ ] One final `improve-code-boundaries` pass confirms the dead flake workflow catalog is gone and the workflow calls one honest Nix image-artifact boundary
- [ ] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required verification step is green

Plan path: `.ralph/tasks/story-31-nix-github-workflow-cache-and-image-artifacts/01-task-create-ultra-simple-nix-github-workflow-with-cached-parallel-builds_plans/2026-04-28-ultra-simple-nix-workflow-plan.md`

NOW EXECUTE
