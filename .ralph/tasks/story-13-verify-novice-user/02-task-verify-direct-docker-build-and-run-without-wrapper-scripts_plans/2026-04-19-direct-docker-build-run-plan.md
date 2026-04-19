# Plan: Verify Direct Docker Build And Run Without Wrapper Scripts

## References

- Task: `.ralph/tasks/story-13-verify-novice-user/02-task-verify-direct-docker-build-and-run-without-wrapper-scripts.md`
- Previous story-13 plan: `.ralph/tasks/story-13-verify-novice-user/01-task-verify-readme-alone-is-sufficient-for-novice-user_plans/2026-04-19-readme-novice-user-plan.md`
- Current Docker quick start:
  - `README.md`
- Existing README contract:
  - `crates/runner/tests/readme_contract.rs`
  - `crates/runner/tests/support/readme_contract.rs`
- Existing direct-container verification:
  - `crates/runner/tests/long_lane.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
  - `crates/runner/tests/support/runner_container_process.rs`
- Existing public CLI contracts:
  - `crates/runner/tests/cli_contract.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public interface and behavior priorities in this turn.
- The currently shipped novice-user container contract is direct `docker build` plus direct `docker run` against the image entrypoint.
- `docker compose up` is not currently part of the documented public contract in `README.md`; do not invent a compose workflow unless the first RED slice proves the docs already promise one.
- This task is about the public user path, not about internal test-only harness wrappers used to audit CockroachDB or MOLT in unrelated end-to-end scenarios.
- The task must prove all of the following at the public boundary:
  - the documented container path uses normal Docker commands directly
  - the container image starts the `runner` binary directly rather than a shell wrapper
  - the documented direct container commands stay aligned with the real CLI surface
- If the first RED slice shows that the current tests cannot prove the public Docker contract without relying on duplicated stringly command assembly or leaking internal wrapper behavior into the assertion boundary, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - the README Docker quick start documents a direct `docker build` and direct `docker run` path
  - the task fails if the documented user path requires a wrapper bash script or shell entrypoint
  - the direct image path works against a real PostgreSQL container using the shipped image entrypoint
  - the documented container subcommands remain aligned with the real public CLI surface
- Lower-priority concerns:
  - documenting an optional compose workflow
  - broad README prose cleanup beyond the direct Docker path

## Problem To Fix

- `README.md` already says the destination runtime is one container and that there is no wrapper shell script in the user path, but there is no focused contract that proves this claim.
- `crates/runner/tests/long_lane.rs` already exercises a real `docker build` plus `docker run` flow, but it is not tied tightly enough to the README-owned direct Docker contract.
- The current test support spreads direct-container knowledge across multiple places:
  - README string checks
  - `runner_image_harness.rs`
  - `runner_container_process.rs`
- That split makes it too easy for documentation, command assembly, and real execution to drift apart without one honest owner for the public Docker contract.

## Interface And Boundary Decisions

- Keep all product CLIs unchanged unless the RED slices expose a real public-surface mismatch.
- Keep the public Docker contract explicit and narrow:
  - `docker build -t cockroach-migrate-runner .`
  - direct `docker run ... validate-config --config <path>`
  - direct `docker run ... run --config <path>`
- Add one dedicated support boundary for the direct Docker contract instead of duplicating Docker command literals across README assertions and harness code.
  - preferred file: `crates/runner/tests/support/runner_docker_contract.rs`
- The support boundary should own:
  - the canonical direct-image build command shape
  - the canonical direct container subcommand and mount conventions used by the README quick start
  - assertions that the image entrypoint is the `runner` binary rather than a shell
- Keep behavior tests focused on public outcomes:
  - `readme_contract.rs` should describe the operator-facing Docker contract
  - long-lane execution should prove the direct image path works against real Docker and PostgreSQL
- If `runner_container_process.rs` or `runner_image_harness.rs` becomes a thin delegator after the extraction, flatten or delete the thinner boundary so there is one honest owner.

## Public Contract To Establish

- One fast README contract fails if the Docker quick start stops using direct Docker commands.
- One fast README contract fails if the Docker quick start introduces a wrapper bash script, `.sh` handoff, or shell-based image entrypoint into the user path.
- One fast contract fails if the documented container subcommands drift away from the real `runner` CLI surface.
- One ignored long-lane scenario proves that the direct container path works end to end:
  - build the image directly from the repo root
  - validate a mounted config directly through the image entrypoint
  - start the runtime directly through the image entrypoint
  - observe healthy startup and helper-table bootstrap against real PostgreSQL

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - direct Docker contract knowledge is duplicated between README string assertions and test harness command assembly
- Required cleanup during execution:
  - extract or centralize the direct Docker command contract so README checks and executable harnesses rely on one typed owner
  - keep `readme_contract.rs` behavior-focused instead of adding more ad hoc string searching
  - avoid two separate helpers assembling nearly identical `docker build` and `docker run` invocations for the same public contract
- Do not add another fake abstraction layer. If the new support file only forwards literals without owning real contract knowledge, flatten it again.

## Files And Structure To Add Or Change

- [x] `README.md`
  - only if a RED slice proves the Docker quick start wording or command shape is incomplete or drifts from the real contract
- [x] `crates/runner/tests/readme_contract.rs`
  - expand Docker quick-start verification around direct build/run and no-wrapper requirements
- [x] `crates/runner/tests/support/readme_contract.rs`
  - only if README section lookup needs richer Docker command assertions
- [x] `crates/runner/tests/long_lane.rs`
  - strengthen the direct-image long lane so it reads as the public Docker contract, not just a generic smoke test
- [x] `crates/runner/tests/support/runner_image_harness.rs`
  - likely reuse or reshape around the typed direct Docker contract
- [x] `crates/runner/tests/support/runner_container_process.rs`
  - only if it still owns distinct runtime lifecycle behavior after Docker command extraction
- [x] `crates/runner/tests/support/runner_docker_contract.rs`
  - preferred new support owner for direct Docker command shape and entrypoint assertions
- [x] `crates/runner/tests/cli_contract.rs`
  - only if execution finds a Docker-documented subcommand not already protected by a real CLI contract
- [x] No product runtime changes are expected
  - if RED exposes a real image-entrypoint or CLI mismatch, fix the real public contract rather than weakening the test

## TDD Execution Order

### Slice 1: Tracer Bullet For The README-Owned Direct Docker Path

- [x] RED: add one failing README contract that requires the Docker quick start to show direct `docker build` and direct `docker run` usage without wrapper-script handoff
- [x] GREEN: make the smallest README or test-support change needed to close the first real gap
- [x] REFACTOR: move Docker quick-start phrase and command-shape assertions behind the dedicated support boundary instead of leaving stringly checks scattered in the test file

### Slice 2: Prove Wrapper Scripts Are Not Part Of The Public Docker Contract

- [x] RED: add the next failing contract that rejects wrapper bash script dependence in the README Docker path and rejects a shell-based image entrypoint for the shipped container
- [x] GREEN: fix only the first real contract gap the assertion exposes
- [x] REFACTOR: keep the no-wrapper checks owned by the Docker contract support boundary rather than duplicated in README and long-lane assertions

### Slice 3: Tie Docker-Documented Subcommands To The Real CLI Surface

- [x] RED: add a failing contract for the first Docker-documented runner subcommand or argument shape that is not already protected by an honest public CLI assertion
- [x] GREEN: strengthen the real CLI contract or the Docker contract support only where drift is actually possible
- [x] REFACTOR: centralize the documented subcommand inventory so README and CLI checks do not duplicate unrelated string literals

### Slice 4: Prove The Direct Image Path Works Against Real Docker

- [x] RED: strengthen the ignored long lane so it executes the direct image build, mounted-config validation, and runtime startup path in the same public shape promised by the README
- [x] GREEN: make only the minimum harness or real-code changes needed for the direct Docker path to pass
- [x] REFACTOR: leave one honest owner for direct Docker command assembly; if `runner_image_harness.rs` and `runner_container_process.rs` overlap after this slice, merge or delete the thinner one

### Slice 5: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so the direct Docker contract has one honest owner and no wrapper-script drift can hide between docs and harnesses

## TDD Guardrails For Execution

- Every new assertion must fail before the supporting code or README change is added.
- Do not satisfy this task by broadening the definition of "direct Docker" to include repo-local wrapper scripts.
- Do not satisfy this task by asserting only on README prose if a real executable Docker contract is missing.
- Do not absorb story-13 task 03 by turning this into a general config-example rewrite.
- Do not absorb future compose work unless the first RED slice proves the current public contract already promises compose.
- Do not swallow Docker command failures, image build failures, or runtime startup failures. They are task failures.

## Boundary Review Checklist

- [x] One honest support boundary owns the direct Docker contract
- [x] `readme_contract.rs` reads as operator behavior, not Docker string plumbing
- [x] Direct build/run command expectations are not duplicated across unrelated harnesses
- [x] Wrapper-script rejection is explicit at the public contract boundary
- [x] The image entrypoint assertion stays on the real image boundary
- [x] No error path is swallowed

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
