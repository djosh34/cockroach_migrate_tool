# Plan: Flatten Verify Image CLI Onto The Explicit `run` Surface

## References

- Task:
  - `.ralph/tasks/story-02-verify-service-ergonomics/task-05-verify-cli-run-subcommand.md`
- Verify CLI and tests:
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Published verify image and operator contracts:
  - `cockroachdb_molt/molt/Dockerfile`
  - `crates/runner/tests/support/operator_cli_surface.rs`
  - `crates/runner/tests/operator_cli_surface_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/support/verify_docker_contract.rs`
  - `crates/runner/tests/support/verify_image_artifact_harness.rs`
- Operator docs and artifacts:
  - `README.md`
  - `artifacts/compose/verify.compose.yml`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This turn is planning-only because task 05 had no `<plan>` pointer and no execution marker.
- The task markdown is stale about one important detail:
  - the vendored Go CLI already exposes `molt verify-service run --config <path>`
  - the remaining inconsistency lives in the published image surface, README snippets, and Rust-side operator contracts
- Repo-level instructions explicitly reject backwards compatibility for greenfield UX work.
  - execution should prefer one clean surface over preserving the current hidden-entrypoint shortcut
- The public operator contract should match the binary contract exactly:
  - `validate-config`
  - `run`
- If the first RED slice proves the image must keep a direct `run` entrypoint for a hard repo-owned reason that cannot be expressed through the shared CLI contract, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Current State Summary

- The Go command tree is already the desired shape:
  - `verify-service`
  - `verify-service validate-config --config <path>`
  - `verify-service run --config <path>`
- The Go help tests already enforce that one-action command layer.
- The published verify image currently hides that layer by baking `run` into `ENTRYPOINT`.
  - `ENTRYPOINT ["/usr/local/bin/molt", "verify-service", "run"]`
- The README verify quick start and `artifacts/compose/verify.compose.yml` therefore still show bare flags:
  - `--log-format json`
  - `--config /config/verify-service.yml`
- The Rust operator surface mirrors that hidden-image contract instead of the real CLI contract.
  - `verify-service-image` is modeled as `max_action_depth: 0`
  - it exposes only `run` and no `validate-config` action

## Improve-Code-Boundaries Focus

- Primary boundary smell:
  - the verify image owns a shadow CLI surface that differs from the actual `verify-service` command tree
- Desired cleanup:
  - remove the image-only zero-depth contract
  - make the image entrypoint stop inventing its own CLI shape
  - let docs, Docker, and Rust contract tests all reflect the same first-party `verify-service` action boundary
- Bold refactor allowance:
  - if the zero-depth `verify-service-image` help contract becomes dead after execution, replace it outright instead of layering compatibility rules on top

## Intended Public Contract After Execution

- Container users invoke the published verify image with explicit actions:
  - `docker run "${VERIFY_IMAGE}" validate-config --config /config/verify-service.yml`
  - `docker run "${VERIFY_IMAGE}" run --config /config/verify-service.yml --log-format json`
- Compose uses the same explicit runtime action:
  - `command: ["run", "--log-format", "json", "--config", "/config/verify-service.yml"]`
- The image root help exposes one visible command layer, like `runner` and `setup-sql`.
- Bare image invocation with only flags is no longer the documented or supported contract.
- The Go binary keeps its existing explicit `run` and `validate-config` behavior.

## Type And Boundary Decisions

- Keep the Go `verifyservice.Command()` structure as the single source of truth for actions.
- Change the image entrypoint to expose `verify-service` rather than `verify-service run`.
  - preferred shape:
    - `ENTRYPOINT ["/usr/local/bin/molt", "verify-service"]`
- Update the Rust operator surface owner so `verify-service-image` becomes a one-level command surface.
  - allowed actions:
    - `validate-config`
    - `run`
  - max action depth:
    - `1`
- Add one explicit contract that bare top-level `--config` is not the verify-service interface anymore.

## Expected Code Shape

- `cockroachdb_molt/molt/Dockerfile`
  - expose `verify-service` as the image entrypoint root
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - keep help coverage
  - add one public-interface test that rejects bare root flags and requires explicit subcommands
- `crates/runner/tests/support/operator_cli_surface.rs`
  - convert the verify image surface from depth `0` to depth `1`
  - define help contracts for `validate-config` and `run`
- `crates/runner/tests/operator_cli_surface_contract.rs`
  - update the expected action set and depth policy
- `crates/runner/tests/verify_image_contract.rs`
  - assert image root help and subcommand help through the new visible command layer
- `crates/runner/tests/support/verify_docker_contract.rs`
  - update the expected image entrypoint JSON
- `crates/runner/tests/support/verify_image_artifact_harness.rs`
  - run help and validation commands through the new entrypoint contract
- `README.md`
  - update verify quick start to use explicit `validate-config` and `run` actions where applicable
- `artifacts/compose/verify.compose.yml`
  - add `run` as the first command token

## Vertical TDD Slices

### Slice 1: Tracer Bullet For The Shared Verify Image Surface

- [ ] RED: tighten the Rust operator CLI surface contract so `verify-service-image` requires:
  - visible action depth `1`
  - allowed actions `validate-config` and `run`
  - root help that lists commands instead of bare flags
- [ ] GREEN: update the Docker entrypoint, operator surface metadata, and verify image harness/contract helpers to satisfy that shared command surface
- [ ] REFACTOR: delete any zero-depth verify-image assumptions left behind
- Stop condition:
  - if this slice proves the image cannot honestly expose both actions through one root without adding another wrapper layer, switch back to `TO BE VERIFIED` and stop immediately

### Slice 2: Lock The Go CLI Boundary

- [ ] RED: add one failing Go test proving `verify-service --config <path>` is not accepted and the user must choose `run` or `validate-config`
- [ ] GREEN: keep or minimally adjust command wiring until the explicit-subcommand contract is enforced cleanly
- [ ] REFACTOR: keep the command root thin and avoid any aliasing path for bare flags

### Slice 3: Align README And Compose With The Real Contract

- [ ] RED: tighten README and artifact-backed contract tests so verify examples require:
  - `validate-config --config /config/verify-service.yml` when validating through the image
  - `run --config /config/verify-service.yml`
  - `verify.compose.yml` commands that begin with `run`
- [ ] GREEN: update `README.md` and `artifacts/compose/verify.compose.yml`
- [ ] REFACTOR: remove wording that implies the verify image has a different CLI model from runner

### Slice 4: Boundary Audit

- [ ] Run one final `improve-code-boundaries` pass with this question:
  - does any image, doc, or contract layer still describe a verify-specific shortcut instead of the shared command tree?
- [ ] If yes, flatten it now instead of preserving another split public surface

### Slice 5: Repository Validation

- [ ] Run `make check`
- [ ] Run `make lint`
- [ ] Run `make test`
- [ ] Do not run `make test-long`

## Expected Outcome

- The verify binary, verify image, README, and Rust operator contracts will all teach the same explicit action boundary.
- The verify image will stop being a special-case CLI surface.
- This task should remove a contract split instead of adding more translation code around it.

Plan path: `.ralph/tasks/story-02-verify-service-ergonomics/task-05-verify-cli-run-subcommand_plans/2026-04-25-verify-cli-run-subcommand-plan.md`

NOW EXECUTE
