# Plan: Single-Binary Container Contract

## References

- Task: `.ralph/tasks/story-02-rust-foundation/02-task-build-single-binary-container-contract.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Design: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Interface And Boundary Decisions

- Keep the user entrypoint direct: the destination image must start the Rust `runner` binary directly, not a shell wrapper.
- Keep one startup contract across local and container usage. The same `runner run --config <path>` interface must work on the host and inside the container.
- Keep container-specific concerns at the edge. Do not create a second "container config" type; reuse validated `RunnerConfig` and only document the mount convention.
- Keep the image single-purpose. The destination image should contain exactly the `runner` binary required for webhook and reconcile startup, not the source bootstrap binary.
- Keep Docker contract tests behavioral. Prefer real `docker build` and `docker run` assertions over inspecting Dockerfile text when proving the user path.
- Use `improve-code-boundaries` to flatten any startup duplication uncovered while wiring container-specific startup summaries or config-path handling. If a container helper type exists only to mirror CLI/runtime state, remove it instead of layering adapters.

## Public Contract To Establish

- A user can build the destination runtime directly with `docker build -t cockroach-migrate-runner .`.
- A user can validate mounted config directly with:
  - `docker run --rm -v <fixture-dir>:/config cockroach-migrate-runner validate-config --config /config/runner.yml`
- A user can start the runtime directly with:
  - `docker run --rm -p 8443:8443 -v <fixture-dir>:/config cockroach-migrate-runner run --config /config/runner.yml`
- The image entrypoint is the `runner` binary itself, so Docker arguments append directly to that binary instead of going through `/bin/sh`.
- The documented mount convention is one mounted config directory, with the runner config and TLS material referenced by mounted paths.
- The README quick-start uses direct Docker commands only. No wrapper bash scripts are part of the novice-user path.

## Files And Structure To Add Or Change

- [x] Root `Dockerfile` for the destination runner image
- [x] `README.md` quick-start section for direct Docker build/run usage and config mounting conventions
- [x] `crates/runner/tests/cli_contract.rs` to strengthen the binary help/startup contract if needed
- [x] `crates/runner/tests/config_contract.rs` or a new integration-style contract test for mounted config behavior
- [x] `crates/runner/tests/long_lane.rs` to add a real Docker build/run contract test in the long lane
- [x] `crates/runner/tests/fixtures/` container-oriented fixture paths if the current fixture does not express the mounted `/config/...` contract clearly
- [x] `crates/runner/src/lib.rs` to keep the startup contract and summaries aligned with the direct binary/container path
- [x] Any runner config/runtime files that need reduction so container startup still flows through one validated config boundary

## TDD Execution Order

### Slice 1: Direct Binary Startup Contract

- [x] RED: add one integration-style test that proves `runner run --config <fixture>` reports a startup contract compatible with direct container execution
- [x] GREEN: minimally adjust the runner startup output or config fixture shape until the direct startup contract is explicit and stable
- [x] REFACTOR: remove any duplicate startup formatting or config projection that makes the CLI boundary shallower than necessary

### Slice 2: Mounted Config Convention

- [x] RED: add one test that validates a mounted-style config path such as `/config/runner.yml` remains the only required startup input
- [x] GREEN: make config fixtures and validation support the mounted-directory convention without inventing a second container-only config schema
- [x] REFACTOR: keep path validation and path display inside the config boundary instead of scattering mount assumptions through CLI and runtime code

### Slice 3: Dockerfile Build Contract

- [x] RED: add one ignored long-lane integration test that runs a real `docker build` for the repository and fails because the image contract does not exist yet
- [x] GREEN: add the minimal root `Dockerfile` that builds the Rust workspace and produces an image whose entrypoint is the `runner` binary
- [x] REFACTOR: keep the image focused on the destination runtime only; if the build stage or copy layout drags in unused artifacts, remove them

### Slice 4: Direct Container Startup Contract

- [x] RED: extend the ignored Docker integration test so `docker run ... validate-config --config /config/runner.yml` and `docker run ... run --config /config/runner.yml` exercise the image without shell wrappers
- [x] GREEN: finish image wiring, fixture layout, and runtime output until the direct Docker commands succeed against the built image
- [x] REFACTOR: collapse any container-only adapter code so the image launches the same runner command path used in host execution

### Slice 5: Novice-User Documentation Contract

- [x] RED: add or tighten one behavioral test expectation and README wording so the direct Docker path is the documented user path, not an inferred one
- [x] GREEN: document the exact build, validate-config, and run commands with the config mount convention in `README.md`
- [x] REFACTOR: remove any stale or indirect wording that points users toward scripts or repository spelunking

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix the first failing lane only
- [x] GREEN: continue until every lane passes cleanly, including the real Docker long-lane contract
- [x] REFACTOR: use `improve-code-boundaries` one more time to remove any container/bootstrap duplication that appeared during implementation

## Boundary Review Checklist

- [x] No wrapper bash script exists in the user path for building or starting the destination runtime
- [x] No container-only config type duplicates `RunnerConfig`
- [x] No Docker-specific argument parsing sits outside the existing `runner` CLI boundary
- [x] No image artifact other than the destination `runner` binary is required at runtime
- [x] No README step requires users to inspect scripts or source to infer startup behavior
- [x] No new startup adapter exists solely to translate between CLI output and Docker expectations

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
