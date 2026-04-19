# Plan: Build A Scratch Verify Image From The Pruned Verify Source

## References

- Task: `.ralph/tasks/story-18-verify-http-image/03-task-build-a-scratch-verify-image-from-the-pruned-verify-source.md`
- Prior source-slice task and plan:
  - `.ralph/tasks/story-18-verify-http-image/01-task-prune-the-codebase-down-to-a-verify-only-source-slice-and-prove-removal.md`
  - `.ralph/tasks/story-18-verify-http-image/01-task-prune-the-codebase-down-to-a-verify-only-source-slice-and-prove-removal_plans/2026-04-19-verify-source-slice-prune-plan.md`
- Prior toolchain/dependency task and plan:
  - `.ralph/tasks/story-18-verify-http-image/02-task-upgrade-the-verify-slice-to-go-1-26-and-bump-all-dependencies.md`
  - `.ralph/tasks/story-18-verify-http-image/02-task-upgrade-the-verify-slice-to-go-1-26-and-bump-all-dependencies_plans/2026-04-19-verify-go-1-26-dependency-upgrade-plan.md`
- Follow-up task:
  - `.ralph/tasks/story-18-verify-http-image/04-task-add-verify-service-config-for-source-destination-tls-and-mode-selection.md`
  - `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs.md`
- Existing image and contract files:
  - `Dockerfile`
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/long_lane.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
  - `crates/runner/tests/support/verify_source_contract.rs`
- Verify-only source slice:
  - `cockroachdb_molt/molt/main.go`
  - `cockroachdb_molt/molt/cmd/root.go`
  - `cockroachdb_molt/molt/cmd/verify/verify.go`
  - `cockroachdb_molt/molt/go.mod`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- Task 03 is packaging only.
  - It must not absorb the dedicated verify-service config contract from task 04.
  - It must not absorb the HTTP job API or single-active-job behavior from task 05.
  - It must not widen into GitHub workflow publication work from story 21.
- The required repository validation lanes for this task are:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` is not part of the default end-of-task gate here.
  - Run it only if execution changes ignored/ultra-long tests or their selection.
- The verify image should be built from the pruned verify slice itself, not from the repository root.
  - Using the full repo as Docker build context would be the wrong boundary for this task.
- The verify image public surface for this task is “verify-only container entrypoint”, not yet “final HTTP service”.
  - Task 05 can still replace that entrypoint later when the HTTP service exists for real.
- If the first red slice shows that the retained Go verify binary cannot be built statically into a scratch image without unexpected runtime payload, or that task 05 necessarily requires a materially different packaging contract right now, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- The repository currently has only one image build path:
  - root `Dockerfile`
  - Rust builder
  - scratch runtime
  - runner-only entrypoint
- The verify-only source boundary is already established under `cockroachdb_molt/molt`.
- The retained root Go command now exposes only `verify`, help, and version.
- The current image-test boundary is in the wrong place for task 03:
  - `runner_docker_contract.rs` owns root-Dockerfile assertions that are specific to the runner image.
  - Growing that module into a fake “all images” abstraction would be a boundary regression.
- The current runner long lane builds and runs a real image against PostgreSQL, but task 03 only needs a lighter default-lane build/run proof for the verify image.

## Interface And Boundary Decisions

- Add a dedicated verify Docker build context at `cockroachdb_molt/molt`.
  - Put the verify image Dockerfile inside that directory.
  - Build it with the verify-slice directory as the Docker context.
- Keep the verify image as a scratch container.
  - No shell.
  - No package manager.
  - No copied source tree in runtime.
  - No config template payload baked into the image yet.
- Build the retained Go module into one static Linux binary.
  - Use Go 1.26 in the builder stage.
  - Derive architecture from Docker build args with explicit `amd64` and `arm64` handling.
  - Fail loudly on unsupported architectures.
- Keep the container public interface verify-only.
  - The runtime entrypoint should invoke the binary directly without a shell.
  - The container should default to the verify command surface rather than exposing a broader general-purpose CLI path.
- Keep verify-image tests in verify-specific support modules.
  - Do not stretch `runner_docker_contract.rs` into a generic image framework.
  - Duplicate a tiny amount of image-specific support if needed instead of introducing a false abstraction.

## Improve-Code-Boundaries Focus

- Primary smell: wrong-place image ownership.
  - Today the only Docker contract support lives in runner-specific test modules.
  - Task 03 should add a verify-specific image contract boundary instead of teaching runner modules about a second product.
- Secondary smell: build-context leakage.
  - A verify image built from the repo root would couple packaging to unrelated Rust code and repo clutter.
  - The Dockerfile and build command should live at the verify slice boundary under `cockroachdb_molt/molt`.
- Tertiary smell: mixed responsibilities in test support.
  - Keep text-contract parsing in one support file and real `docker build` / `docker run` orchestration in another.
  - Do not hide a one-off local check behind a pile of generic helper layers.

## Public Contract To Establish

- A dedicated verify image can be built directly from `cockroachdb_molt/molt`.
- The verify image runtime is `FROM scratch`.
- The runtime filesystem contains only the compiled verify binary payload needed for this stage.
- The verify image does not bundle unrelated source, shell tooling, or runner-specific runtime content.
- The verify image starts the binary directly, not through `/bin/sh` or wrapper scripts.
- Running the built image with `--help` exercises the verify command surface only.
- Repository tests fail if the verify image Dockerfile drifts back to repo-root build context, stops being scratch-based, or starts carrying extra runtime payload.

## Files And Structure To Add Or Change

- [x] `cockroachdb_molt/molt/Dockerfile`
  - new verify-image Dockerfile rooted in the verify-only source slice
- [x] `crates/runner/tests/ci_contract.rs`
  - add text-level verify image contract coverage alongside the existing repo CI contracts
- [x] `crates/runner/tests/verify_image_contract.rs`
  - add default-lane build/run tests for the verify image
- [x] `crates/runner/tests/support/verify_docker_contract.rs`
  - new verify-specific Dockerfile/entrypoint/runtime-file contract owner
- [x] `crates/runner/tests/support/verify_image_harness.rs`
  - new verify-image build/run/export harness for default-lane tests

## TDD Execution Order

### Slice 1: Tracer Bullet For The Verify Dockerfile Boundary

- [x] RED: add one failing text-contract test asserting that a verify Dockerfile exists under `cockroachdb_molt/molt`, uses a scratch runtime stage, and does not rely on a shell entrypoint
- [x] GREEN: add the verify Dockerfile in that location with a scratch runtime and direct binary entrypoint
- [x] REFACTOR: keep all Dockerfile path and string-shape assertions inside `verify_docker_contract.rs`

### Slice 2: Build The Image From The Verify-Only Slice

- [x] RED: add one failing default-lane test that runs `docker build` against `cockroachdb_molt/molt` and expects a real verify image tag to be produced
- [x] GREEN: make the Dockerfile build cleanly on the local architecture from the vendored verify slice only
- [x] REFACTOR: keep build argument assembly and image-tag generation inside `verify_image_harness.rs`, not inline in the test body

### Slice 3: Prove The Public Container Surface Is Verify-Only

- [x] RED: add one failing default-lane test that runs the built image with `--help` and asserts the output is the verify command surface, not the removed fetch surface and not the runner surface
- [x] GREEN: set the image entrypoint so container invocation stays verify-only without wrapper scripts
- [x] REFACTOR: keep help-output assertions in `verify_docker_contract.rs` so later HTTP-service changes update one contract owner

### Slice 4: Prove Runtime Payload Minimality

- [x] RED: add one failing default-lane test that exports the image filesystem and asserts only the intended runtime payload is present
- [x] GREEN: keep the scratch runtime stage to the compiled binary only, with no source files, shell binaries, or extra runtime clutter copied in
- [x] REFACTOR: represent the allowed runtime paths in one verify-image contract helper rather than scattering raw path checks across tests

### Slice 5: Builder-Stage Source Boundary

- [x] RED: add one failing text-contract assertion that the verify Dockerfile copies from the verify-slice context only and does not depend on repository-root Rust sources
- [x] GREEN: keep the build rooted at `cockroachdb_molt/molt` and copy only what the retained Go module needs
- [x] REFACTOR: keep builder-stage boundary rules narrow and intention-based instead of snapshotting the full Dockerfile

### Slice 6: Repository Validation Lanes

- [x] RED: run `make check`, `make lint`, and `make test`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to ensure verify image support stayed isolated instead of accreting into runner-specific modules

## TDD Guardrails For Execution

- Start with a failing contract or integration test before changing the corresponding packaging boundary.
- Keep tests focused on public behavior:
  - Dockerfile location and shape
  - buildability from the verify slice
  - container entrypoint behavior
  - runtime filesystem payload
- Do not invent a generic multi-image contract framework unless execution reveals real shared ownership.
- Do not move this task into HTTP territory early.
  - no static config surface yet
  - no HTTP handlers yet
  - no job-state API yet
- Do not build the verify image from the repository root.
- Do not silently accept extra runtime payload “just in case”.
  - This task explicitly wants a minimal scratch image.
- Do not skip or soften failing Docker-based tests.
  - If the environment cannot build or run the image, execution must fail loudly rather than claiming confidence.

## Boundary Review Checklist

- [x] The verify Dockerfile lives under `cockroachdb_molt/molt`
- [x] The verify build context is the verify-only source slice, not repo root
- [x] The verify image runtime uses `FROM scratch`
- [x] The verify image entrypoint executes directly without a shell or wrapper
- [x] The verify image runtime filesystem contains only the intended binary payload
- [x] Verify image test support lives in verify-specific modules, not in `runner_docker_contract.rs`
- [x] No runner-specific or HTTP-service-specific behavior leaks into task 03

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] If execution changes ignored/ultra-long tests or their selection: `make test-long`
- [x] One final `improve-code-boundaries` pass after all required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after the required lanes pass

Plan path: `.ralph/tasks/story-18-verify-http-image/03-task-build-a-scratch-verify-image-from-the-pruned-verify-source_plans/2026-04-19-verify-scratch-image-plan.md`

NOW EXECUTE
