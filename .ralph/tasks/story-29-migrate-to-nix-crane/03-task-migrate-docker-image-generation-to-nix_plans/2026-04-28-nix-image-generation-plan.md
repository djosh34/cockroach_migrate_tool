# Plan: Move Runtime Image Generation Behind Flake-Native Nix Outputs

## References

- Task:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix.md`
- Previous story step:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane_plans/2026-04-28-migrate-build-run-test-lint-to-crane-plan.md`
- Next story steps that must stay out of scope:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/05-task-enable-development-without-host-nix.md`
- Current packaging and image surfaces:
  - `flake.nix`
  - `Dockerfile`
  - `cockroachdb_molt/molt/Dockerfile`
  - `.github/workflows/image-catalog.yml`
  - `.github/workflows/publish-images.yml`
- Current image contract tests and harnesses:
  - `crates/runner/tests/image_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/support/runner_image_artifact_harness.rs`
  - `crates/runner/tests/support/verify_image_artifact_harness.rs`
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/runner/tests/support/published_image_refs.rs`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`
- Smell set for this task:
  - `.ralph/tasks/smells/2026-04-28-story-29-nix-image-generation-boundaries.md`

## Planning Assumptions

- This is a planning-only turn because the task had no linked plan artifact and no execution marker.
- Execution must use vertical Red-Green TDD:
  - one failing public contract at a time
  - one minimal code change to make that contract green
  - refactor only after the current slice is green
- The public behavior to verify is not Dockerfile text. The public behavior is:
  - Nix can build each runtime image
  - Docker can load each Nix-built image
  - each image still exposes the same entrypoint and runtime contract
  - the runtime filesystem stays minimal
- The final required repo gates for this task remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` must not be used as the default end-of-task gate here.
  - Focused ignored image tests are allowed during red-green execution because this task owns image generation behavior.
  - Full `make test-long` remains reserved for story-end validation or an explicit long-lane task.
- CI workflow migration is explicitly out of scope for this task.
  - This task may update shared image metadata only if needed to keep image identity honest.
  - It must not claim the GitHub workflow is migrated; that belongs to Task 04.
- If execution proves that a truthful minimal runtime image cannot be produced from Nix-built artifacts without reintroducing a second non-Nix compilation path, this plan is wrong and must be switched back to `TO BE VERIFIED` immediately.
- If execution proves that the current flake package split forces the verify image to ship the shell wrapper instead of the real `molt` binary, this plan must be switched back to `TO BE VERIFIED` instead of sneaking a shell into production.

## Current State Summary

- `flake.nix` currently owns build, lint, test, and app outputs, but it does not own runtime image outputs yet.
- Production image generation is still split across Dockerfiles:
  - root `Dockerfile` builds and packages the Rust `runner`
  - `cockroachdb_molt/molt/Dockerfile` builds and packages the Go `molt` binary for `verify-service`
- Image contract tests currently prove behavior by running real `docker build` against those Dockerfiles.
- Multiple test harnesses duplicate Dockerfile-oriented image assembly logic:
  - `runner_image_artifact_harness`
  - `verify_image_artifact_harness`
  - `verify_image_harness`
  - `published_image_refs`
- There is also dead implementation-coupled test support:
  - `crates/runner/tests/support/rust_workspace_image_cache_contract.rs`
  - it asserts Dockerfile text markers rather than runtime behavior and currently has no callers
- The novice/public README is already centered on published images, not local image builds.
  - That means contributor-facing local Nix image build notes should stay out of the public quick start unless a public contract truly changed.

## Improve-Code-Boundaries Focus

- Primary boundary problem:
  - image identity and runtime contract live in the tests and docs
  - binary compilation lives in `flake.nix`
  - image assembly lives in Dockerfiles
  - Dockerfile-specific knowledge is repeated in several harnesses
- That is classic wrong-place ownership:
  - Nix owns the build graph
  - Dockerfiles still own the production runtime artifact shape
  - tests then have to understand both worlds
- Desired boundary after execution:
  - `flake.nix` owns both binary build and runtime image assembly
  - tests treat Docker as a consumer only:
    - load image
    - inspect image
    - run image
    - export image
  - tests no longer know or care about Dockerfile paths or build contexts
- Secondary boundary problem:
  - runner and verify image harnesses each reimplement slightly different versions of:
    - build/load image
    - inspect image
    - export runtime filesystem
    - cleanup tags
  - `published_image_refs.rs` duplicates the same concern again
- Desired cleanup:
  - create one deeper shared test support boundary for "load a Nix-built image into Docker and return the tag"
  - keep runner-specific and verify-specific assertions near their own contract files
- Important non-goal:
  - do not create a generic image abstraction that hides the runtime contract.
  - keep shared support limited to artifact loading and cleanup, not domain assertions.

## Intended Public Contract After Execution

- Nix provides first-class runtime image outputs for both supported images.
- Each runtime image output is built from Nix-managed binary artifacts rather than Dockerfile-local compilers.
- Docker can consume each image artifact without rebuilding:
  - load into the local daemon for tests and manual inspection
  - inspect entrypoint metadata
  - run the image directly
  - export the filesystem when needed by tests
- The runtime contracts stay unchanged:
  - runner entrypoint remains `["/usr/local/bin/runner"]`
  - verify entrypoint remains `["/usr/local/bin/molt","verify-service"]`
  - runner runtime filesystem remains minimal and contains only the runner payload
  - verify runtime filesystem remains minimal and contains only the `molt` payload
- The verify image must be assembled from the real `molt` binary, not from the shell-wrapper `verify-service` app derivation.
- Obsolete Dockerfile-based production image generation is removed from the local/test/canonical workflow.
  - Root `Dockerfile` and `cockroachdb_molt/molt/Dockerfile` should be deleted unless execution proves one is still required for Task 05's no-host-Nix fallback.
  - If the fallback truly needs a Dockerfile, it must be a new explicitly development-only boundary in Task 05, not a retained production artifact here.
- The task notes for the execution turn must record the exact build/load/inspect commands used.

## Expected Code Shape

- `flake.nix`
  - add first-class image packages, likely named along the lines of:
    - `runner-image`
    - `verify-image`
  - introduce one local helper in the flake for assembling minimal runtime images from built binaries
  - keep image metadata, entrypoint, and port exposure in the Nix layer rather than scattering it through tests
- `crates/runner/tests/support/`
  - add one shared support file for loading Nix image artifacts into Docker
  - update existing runner/verify harnesses to depend on that support instead of `docker build`
- `crates/runner/tests/image_contract.rs`
  - keep behavioral assertions, but make the RED path fail on missing/wrong Nix image outputs rather than missing Dockerfiles
- `crates/runner/tests/verify_image_contract.rs`
  - same migration for verify
- `crates/runner/tests/support/published_image_refs.rs`
  - build/load local test images from the flake outputs instead of Dockerfiles
- Delete:
  - `Dockerfile`
  - `cockroachdb_molt/molt/Dockerfile`
  - `crates/runner/tests/support/rust_workspace_image_cache_contract.rs`
  - any other Dockerfile-only helper left with no honest runtime contract
- Documentation/task notes:
  - add contributor-facing notes only where needed for the exact Nix image commands
  - keep novice/public README focused on published images

## Type And Interface Decisions

- Prefer one explicit shared test boundary such as:
  - `NixImageArtifactHarness`
  - or a similarly narrow helper name
- That helper should own only:
  - building a flake package
  - loading the resulting image into Docker
  - returning the loaded tag or image id
  - cleanup
- It should not own:
  - runner entrypoint assertions
  - verify module-version assertions
  - runtime filesystem expectations
- Keep the current contract types where they are already honest:
  - `RunnerDockerContract`
  - `verify_docker_contract_support`
- But remove API surfaces that encode Dockerfile-specific build knowledge, such as:
  - Dockerfile path lookup
  - build context lookup
  - cache-marker text assertions

## Vertical TDD Slices

### Slice 1: Runner Image Tracer Bullet

- [ ] RED:
  - change one ignored runner image contract path so it tries to consume a flake image output instead of running `docker build`
  - the first failing proof should be simple:
    - Nix builds the runner image artifact
    - Docker loads it
    - the tag exists locally
- [ ] GREEN:
  - add the minimal `flake.nix` runner image output and shared test loader support needed to make that one proof pass
- [ ] REFACTOR:
  - remove the root Dockerfile dependency from runner image harness support
- Stop condition:
  - if a minimal truthful runner image cannot be produced from a Nix-built binary without inventing a second hidden build path, switch back to `TO BE VERIFIED`

### Slice 2: Runner Runtime Contract Preservation

- [ ] RED:
  - re-enable the existing runner image assertions one at a time against the Nix-built image:
    - direct entrypoint
    - minimal runtime filesystem
    - `validate-config --log-format json`
- [ ] GREEN:
  - fill in the runner image metadata in Nix:
    - entrypoint
    - any required exposed ports
    - copied runtime binary location
  - if runner needs a runtime-specific derivation to stay minimal, keep it as a first-class flake output rather than a Dockerfile-local build
- [ ] REFACTOR:
  - keep runtime-root assembly inside one Nix helper instead of duplicating copy/install steps per image

### Slice 3: Verify Image Tracer Bullet

- [ ] RED:
  - move one ignored verify image contract path off Dockerfile build and onto a flake image output
  - the first failing proof should confirm the verify image can be built and loaded through Nix as well
- [ ] GREEN:
  - add the verify image output in `flake.nix`
  - assemble it from the real `molt` binary, not the shell wrapper app
- [ ] REFACTOR:
  - reuse the same shared load/cleanup helper used by runner image tests
- Stop condition:
  - if the only way to keep the current verify entrypoint is to ship shell glue in the image, switch back to `TO BE VERIFIED`

### Slice 4: Verify Runtime Contract Preservation

- [ ] RED:
  - restore the existing verify image behavior checks one by one against the Nix-built image:
    - direct `molt verify-service` entrypoint
    - module-version floor checks
    - minimal runtime filesystem
    - `validate-config --log-format json`
- [ ] GREEN:
  - finish verify image metadata and runtime-root assembly until those checks pass
- [ ] REFACTOR:
  - keep module/version assertions in the verify harness and keep image-loading mechanics shared

### Slice 5: Novice And Published-Image Test Boundary

- [ ] RED:
  - update one novice/published-image test path so local image refs come from Nix outputs instead of Dockerfile builds
  - make it fail first where `published_image_refs.rs` still shells out to `docker build`
- [ ] GREEN:
  - migrate `published_image_refs.rs` to the shared Nix image loader
  - ensure the README-driven local contracts still run against the same runtime image behavior
- [ ] REFACTOR:
  - delete Dockerfile-only helpers and any now-dead duplicated build code

### Slice 6: Manual Verification Surface And Notes

- [ ] During execution, record the exact commands used for:
  - building each image with Nix
  - loading each image into Docker
  - inspecting entrypoints
  - validating config through each image
  - exporting or saving images for artifact verification
- [ ] Keep those commands in task execution notes instead of muddying the public novice README unless a public operator workflow truly changed
- [ ] Manual verification for this task must cover:
  - runner image builds successfully
  - verify image builds successfully
  - each image loads into Docker without rebuild
  - each image starts through its expected command path

### Slice 7: Final Boundary Cleanup And Required Repo Gates

- [ ] Run one final `improve-code-boundaries` pass with these questions:
  - does any test still know a Dockerfile path or build context?
  - does any production image path still compile code outside the Nix graph?
  - does any runtime image still depend on a shell wrapper when the contract says it should not?
  - is there any dead Dockerfile/cache-shape test helper left behind?
- [ ] Run the required repo gates:
  - `make check`
  - `make lint`
  - `make test`
- [ ] Use focused ignored image-contract test runs as needed during implementation, but do not run full `make test-long` for this task

## Execution Guardrails

- Do not write tests that assert Dockerfile text, file names, or cache-marker strings.
- Do not keep Dockerfiles around "just until CI is migrated" if the local/test/runtime contract can already move to Nix.
- Do not smuggle image assembly logic into shell scripts or workflow steps when `flake.nix` should own it.
- Do not use the `verify-service` shell app derivation as the verify runtime payload.
- Do not broaden README's public quick start into contributor-only local build documentation.
- Do not weaken the existing runtime contract just to fit an easier Nix image shape.

## Expected Outcome

- The repository has one honest owner for production image assembly: the flake.
- Docker is reduced to a runtime consumer in tests and manual verification, not a builder of canonical artifacts.
- Runner and verify image contracts still prove the same user-visible behavior, but now against Nix-built images.
- Dockerfile-based production image generation disappears from the canonical local/test flow.
- The next task can migrate CI publication to consume these Nix image outputs instead of rebuilding through Dockerfiles.

Plan path: `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix_plans/2026-04-28-nix-image-generation-plan.md`

NOW EXECUTE
