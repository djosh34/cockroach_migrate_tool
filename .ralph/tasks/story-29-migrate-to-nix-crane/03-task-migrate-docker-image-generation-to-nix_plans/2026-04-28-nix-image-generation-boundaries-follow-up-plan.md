# Plan: Flatten The Remaining Nix Image Boundary Smells

## References

- Active smell task:
  - `.ralph/tasks/smells/2026-04-28-story-29-nix-image-generation-boundaries.md`
- Story task that already moved image generation into Nix:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix.md`
  - `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix_plans/2026-04-28-nix-image-generation-plan.md`
- Follow-on CI task that must stay out of scope:
  - `.ralph/tasks/story-29-migrate-to-nix-crane/04-task-migrate-ci-to-nix-only.md`
- Current flake and workflow surfaces touched by the smell:
  - `flake.nix`
  - `.github/workflows/publish-images.yml`
  - `.github/workflows/AGENTS.md`
- Current image contract tests and support:
  - `crates/runner/tests/image_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/runner/tests/support/nix_image_artifact_harness.rs`
  - `crates/runner/tests/support/runner_image_artifact_harness.rs`
  - `crates/runner/tests/support/verify_image_artifact_harness.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/runner/tests/support/published_image_refs.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/support/verify_docker_contract.rs`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This is a planning-only turn because the active smell task had no linked follow-up plan and no execution marker.
- The task markdown is sufficient approval for this planning turn.
- The next turn must execute through vertical Red-Green TDD:
  - one failing public behavior at a time
  - one minimal fix to make it green
  - refactor only after the current slice is green
- The public contracts here are executable image behaviors, not implementation strings:
  - Nix-built images can be provisioned into Docker
  - runner and verify image contracts still pass against those provisioned images
  - novice/public-image flows still consume the same provisioned images without rebuilding
- The final repo gates for the execution turn remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` is not a default end-of-task gate here.
  - Focused ignored image tests are allowed during red-green execution because this smell task owns image contract support.
  - Full `make test-long` remains reserved for story-end or explicitly long-lane tasks.
- Workflow-local testing rules from `.github/workflows/AGENTS.md` still apply.
  - Do not invent YAML string tests for workflow files.
  - If workflow edits are needed here, they must remain small and be validated through the repo gates plus the existing hosted workflow boundary already owned by task 04.
- If execution proves the remaining duplication cannot be removed without changing the honest public image contracts, switch this file back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- `flake.nix` already owns the runtime image outputs and the publish-image metadata.
- The old Dockerfile cache-marker helper named in Smell 10 is already gone.
  - `crates/runner/tests/support/rust_workspace_image_cache_contract.rs` is missing.
  - The execution turn should treat Smell 10 as a stale smell reference and verify there is no surviving dead helper rather than searching for that exact file.
- The remaining live mud is test-support duplication around the same boundary:
  - build/load a Nix image archive
  - tag it for a test-specific local ref
  - inspect it
  - export its filesystem or copy binaries out
  - clean up the temporary tag/container state
- That lifecycle is currently split across:
  - `nix_image_artifact_harness.rs`
  - `runner_image_artifact_harness.rs`
  - `verify_image_artifact_harness.rs`
  - `published_image_refs.rs`
  - and, indirectly, the runtime harnesses that each provision their own image tag before exercising behavior
- `publish-images.yml` now consumes flake-owned metadata, but the flake still exposes separate string fields such as:
  - `loaded_image_ref`
  - `manifest_key`
  - per-image repository names
  - package attr names
- That workflow metadata is already in better shape than before task 04, but the smell remains valid if image identity is duplicated between the flake outputs and test-support call sites instead of owned by one typed image-spec boundary.

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - the repo has one honest image producer in `flake.nix`
  - but test support still has several near-copies for "turn flake image output into a disposable local Docker tag"
- Desired boundary after execution:
  - one shared image-spec + provisioning boundary owns:
    - flake package attr
    - canonical loaded image ref
    - context label for failure messages
    - reusable build/load/tag/cleanup plumbing
  - runner-specific and verify-specific harnesses own only the assertions and runtime behavior unique to that image
- Secondary boundary smell:
  - local/public image refs are reassembled in multiple places from ad hoc strings
  - that makes image identity stringly across support code
- Desired cleanup:
  - centralize static image identity into one narrow support type
  - let harnesses request `runner` or `verify` by spec rather than rebuilding the same strings manually
- Important non-goals:
  - do not create a giant generic image framework
  - do not hide runner/verify runtime assertions behind a single abstraction
  - do not move runtime-contract assertions into `flake.nix`

## Public Contract To Preserve

- `runner_image_builds_from_the_root_runtime_slice`
- `runner_image_exposes_a_direct_runtime_only_entrypoint`
- `runner_image_runtime_filesystem_contains_only_the_runner_payload`
- `runner_image_validate_config_supports_json_operator_logs`
- `verify_image_builds_from_the_verify_slice`
- `verify_image_exposes_a_direct_verify_service_entrypoint`
- `verify_image_embeds_pgx_at_or_above_the_security_floor`
- `verify_image_embeds_grpc_at_or_above_the_security_floor`
- `verify_image_keeps_x_crypto_out_of_vulnerable_runtime_versions`
- `verify_image_runtime_filesystem_contains_only_the_binary_payload`
- `verify_image_validate_config_supports_json_operator_logs`
- The novice/published-image flows in `novice_registry_only_contract.rs` must keep consuming local provisioned images without any Docker rebuild step.

## Type And Interface Decisions

- Introduce one narrow shared image-spec owner in `crates/runner/tests/support/`.
  - Preferred shape:
    - `struct NixTestImageSpec { package_attr, loaded_image_ref, image_label }`
    - plus named constructors or constants for `runner` and `verify`
- The shared Nix support should own:
  - build selector generation
  - `nix build --no-link --print-out-paths`
  - `docker load`
  - `docker tag`
  - cleanup of temporary test tags when appropriate
- The shared Nix support must not own:
  - runner entrypoint assertions
  - verify module-version assertions
  - runtime `docker run` command shapes
  - README/novice contract behavior
- Prefer one helper for common Docker image lifecycle steps that are currently duplicated in runner and verify artifact harnesses:
  - inspect-if-present
  - create/export/remove temporary container around an image
  - maybe extract-one-file from image if that reduces duplication honestly
- If that helper would become too broad, keep the split and only centralize the parts that are truly identical.

## Vertical TDD Slices

### Slice 1: Shared Image Spec Tracer Bullet

- [ ] RED:
  - change one existing public image contract path so it requests a shared `runner` image spec instead of open-coding package attr and loaded ref strings
  - make the failure explicit where the shared spec/helper does not exist yet
- [ ] GREEN:
  - add the minimal shared image-spec owner and wire the runner artifact harness through it
- [ ] REFACTOR:
  - remove the now-duplicated runner-specific package/tag strings from `runner_image_artifact_harness.rs`

### Slice 2: Verify Image Uses The Same Provisioning Boundary

- [ ] RED:
  - move one verify image contract path onto the shared image spec/helper
  - let it fail where verify still duplicates its own package/tag identity
- [ ] GREEN:
  - wire `verify_image_artifact_harness.rs` through the same shared Nix provisioning boundary
- [ ] REFACTOR:
  - delete duplicated verify-specific build/load/tag plumbing that now overlaps exactly with the runner path

### Slice 3: Flatten The Repeated Image Lifecycle Helpers

- [ ] RED:
  - pick one repeated lifecycle behavior that exists in both image artifact harnesses, preferably:
    - create container from image
    - export filesystem
    - remove temporary container
  - let the second contract continue failing until the shared lifecycle helper exists
- [ ] GREEN:
  - extract the minimal shared helper into the Nix image support boundary
  - make both runner and verify artifact harnesses use it
- [ ] REFACTOR:
  - keep image-specific assertions in their image-specific harnesses
  - remove only the duplicated lifecycle mechanics

### Slice 4: Centralize Published Test Image Refs

- [ ] RED:
  - tighten one novice registry-only flow so it must source image identity from the new shared spec instead of duplicating the same strings in `published_image_refs.rs`
- [ ] GREEN:
  - migrate `published_image_refs.rs` to the shared image-spec/provisioning boundary
- [ ] REFACTOR:
  - ensure the novice/public-image support still owns only lazy singleton tag creation and unique suffixing, not the flake image identity itself

### Slice 5: Dead Helper Sweep And Stale Smell Closure

- [ ] Verify during execution that no dead Dockerfile/cache-shape helper remains reachable from tests.
- [ ] If any dead support file remains, delete it in the smallest honest slice.
- [ ] If no such helper remains, update the smell task notes/checklist to record that Smell 10 was already resolved and this pass closed the remaining live boundary duplication instead.

### Slice 6: Broad Validation And Final Boundary Sweep

- [ ] Run the focused ignored image-contract tests needed during red-green development.
- [ ] Run `make check`.
- [ ] Run `make lint`.
- [ ] Run `make test`.
- [ ] Do one final `improve-code-boundaries` sweep:
  - is image identity owned in one place?
  - does any harness still duplicate package attr or loaded image ref strings?
  - does any novice/public image path still rebuild instead of provision?
  - did we avoid inventing a generic abstraction that hides the real runtime contracts?

## Execution Guardrails

- Do not write tests that assert flake text, YAML text, or string markers in support files.
- Do not reintroduce Dockerfile-aware helpers or cache-shape helpers.
- Do not keep two competing image-identity registries in tests and in `flake.nix`.
- Do not weaken explicit panic/error context while deduplicating command helpers.
- Do not silently ignore `docker`/`nix` cleanup failures.
- Do not touch hosted workflow behavior here unless the test-support boundary truly requires a matching metadata cleanup.

## Expected Outcome

- The remaining image-boundary mud is flattened behind one honest shared Nix image support seam.
- Runner and verify harnesses stop repeating the same provisioning and lifecycle mechanics.
- Novice/public-image tests continue to prove real behavior while depending on the same shared image identity source.
- The stale dead-helper smell is either explicitly confirmed as already resolved or removed if any remnant still exists.
- The repository ends this pass with fewer test-support files owning image assembly knowledge and no loss of public contract coverage.

## Design Re-Verified

- The current public contracts already sit at the right behavioral level.
- The missing cleanup is structural, not contractual:
  - image identity is too stringly across support files
  - lifecycle plumbing is duplicated across harnesses
- That means the next execution turn should start with a small RED slice in support code, not with a product-facing contract rewrite.

Plan path: `.ralph/tasks/story-29-migrate-to-nix-crane/03-task-migrate-docker-image-generation-to-nix_plans/2026-04-28-nix-image-generation-boundaries-follow-up-plan.md`

NOW EXECUTE
