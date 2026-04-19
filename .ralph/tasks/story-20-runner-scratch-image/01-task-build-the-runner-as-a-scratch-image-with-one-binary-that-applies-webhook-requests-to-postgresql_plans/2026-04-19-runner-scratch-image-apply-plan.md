# Plan: Scratch Runner Image With Direct PostgreSQL Apply Contract

## References

- Task: `.ralph/tasks/story-20-runner-scratch-image/01-task-build-the-runner-as-a-scratch-image-with-one-binary-that-applies-webhook-requests-to-postgresql.md`
- Related prior plans:
  - `.ralph/tasks/story-02-rust-foundation/02-task-build-single-binary-container-contract_plans/2026-04-18-single-binary-container-contract-plan.md`
  - `.ralph/tasks/story-16-runtime-split-removals/01-task-remove-runner-source-cockroach-access-and-config_plans/2026-04-19-runner-source-access-removal-plan.md`
  - `.ralph/tasks/story-06-destination-ingest/02-task-persist-row-batches-into-helper-shadow-tables_plans/2026-04-18-row-batch-helper-persistence-plan.md`
- Follow-on task:
  - `.ralph/tasks/story-20-runner-scratch-image/02-task-enforce-the-runner-postgresql-only-runtime-contract.md`
- Current runner runtime surface:
  - `Dockerfile`
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/payload.rs`
  - `crates/runner/src/webhook_runtime/routing.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
- Current runner contract coverage:
  - `crates/runner/tests/ci_contract.rs`
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
  - `crates/runner/tests/long_lane.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown is treated as approval for the public interface and behavior priorities in this turn.
- The existing `runner` binary, root `Dockerfile`, and separate `setup-sql` image are the approved product split.
- This task is not allowed to reintroduce setup, verify, shell-wrapper, or multi-binary behavior.
- This task should build on the existing host-level webhook/reconcile behavior instead of inventing a second runtime path for containers.
- Story-20 task 02 is the follow-on for stronger source/verify regression guards. Task 01 should still leave the runner image and apply path explicit enough that task 02 can harden it, not rediscover it.
- If the first RED slice proves the current runtime must change its public command surface, add a second binary, or drop required destination-side bootstrap/reconcile behavior in order to satisfy the task honestly, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - the published runner image remains a scratch image with one direct `runner` binary entrypoint
  - the runtime filesystem is minimal and does not smuggle in shell helpers, setup tools, or verify artifacts
  - the public `runner` surface inside the image is runtime-only: `validate-config` and `run`
  - the public `run` path continues to accept webhook requests and apply them into PostgreSQL through the destination runtime flow
- Lower-priority concerns:
  - preserving the current long-lane-only location of all image assertions
  - preserving stringly internal routing shapes when a typed table boundary can carry the same meaning with less mud

## Problem To Fix

- The core runtime shape already exists:
  - `Dockerfile` builds a scratch image
  - the image entrypoint is `/usr/local/bin/runner`
  - `runner` only exposes `validate-config` and `run`
  - host-level `webhook_contract` and `reconcile_contract` already prove PostgreSQL apply behavior
- The missing gap is public proof at the image boundary:
  - the default suite proves the Dockerfile text shape, but not the built runtime filesystem payload
  - the ignored long lane builds and starts the image, but it stops at health and helper-table bootstrap instead of proving the image remains runtime-only and tied to the apply flow
- There is also one internal boundary smell in the webhook path:
  - `payload::SourceMetadata` renders `schema.table` into a `String`
  - `runtime_plan::MappingRuntimePlan` stores helper tables behind `BTreeMap<String, HelperShadowTablePlan>`
  - `routing` turns typed source fields into string labels and then immediately resolves them back to a helper-table plan
  - this is display-boundary drift and wrong-place string plumbing

## Interface And Boundary Decisions

- Keep one runtime binary only:
  - `runner validate-config --config <path>`
  - `runner run --config <path>`
- Keep the root `Dockerfile` as the single owner of the runtime image.
- Keep the image scratch and direct-entrypoint:
  - no shell wrapper
  - no second binary
  - no setup-sql or verify payload
- Prove the runtime image in the default lane with artifact-style tests:
  - image exists
  - entrypoint is direct
  - help surface is runtime-only
  - exported filesystem contains only the runner payload and container metadata expected from a scratch image
- Keep apply-path behavior tested through the real public runtime interface rather than by calling internal helpers directly.
- Flatten the routing boundary by replacing raw `schema.table` string lookup with a typed source-table key.
  - Preferred shape:
    - parse source table identity into `QualifiedTableName` or one dedicated source-table value type
    - key `MappingRuntimePlan::helper_tables` by that typed key
    - route rows to `HelperShadowTablePlan` without `table_label()` string soup
  - Reason:
    - this makes the webhook routing module a deeper boundary and keeps rendering for error messages only

## Improve-Code-Boundaries Focus

- Primary smell: display/string boundary drift in webhook routing.
  - Delete `SourceMetadata::table_label() -> String`.
  - Delete `MappingRuntimePlan::helper_table(&str)`.
  - Keep table identity typed until an error message or SQL render truly needs a string.
- Secondary smell: image proof currently split between Dockerfile-text assertions and an ignored long-lane harness.
  - Extract a small artifact harness for the built runner image rather than forcing every image assertion through the heavy network/runtime harness.
- Do not add compatibility shims or duplicate container-only config shapes to satisfy these tests.

## Public Contract To Establish

- A built runner image is a scratch runtime image with exactly one direct binary entrypoint.
- The image exposes only the runtime command surface:
  - `validate-config`
  - `run`
- The image runtime filesystem contains no shell bootstrap, verify, or setup payload.
- The public runtime path still applies incoming webhook requests into PostgreSQL through the destination-owned runtime flow.
- README and existing contract helpers continue to describe the runner image as runtime-only and the setup/verify images as separate concerns.

## Files And Structure To Add Or Change

- [x] `.ralph/tasks/story-20-runner-scratch-image/01-task-build-the-runner-as-a-scratch-image-with-one-binary-that-applies-webhook-requests-to-postgresql.md`
  - add this plan path
- [x] `crates/runner/tests/image_contract.rs`
  - add default-lane runner image artifact tests for build, entrypoint, help surface, and minimal filesystem payload
- [x] `crates/runner/tests/support/runner_image_artifact_harness.rs`
  - add a small artifact-only docker harness modeled on the verify-image harness, without runtime/network setup
- [x] `crates/runner/tests/support/runner_docker_contract.rs`
  - extend contract helpers with runtime filesystem assertions shared by the new image tests
- [x] `crates/runner/tests/long_lane.rs`
  - keep the ignored real runtime lane aligned with the new artifact helpers; only strengthen it if execution genuinely changes the long-lane contract
- [x] `crates/runner/src/webhook_runtime/payload.rs`
  - replace stringly source-table rendering with a typed source-table identity
- [x] `crates/runner/src/runtime_plan.rs`
  - key helper-table lookup by the typed table identity instead of raw labels
- [x] `crates/runner/src/webhook_runtime/routing.rs`
  - route batches through the typed table identity and keep error rendering late
- [x] `crates/runner/src/webhook_runtime/persistence.rs`
  - adjust only if the typed routing cleanup changes the batch shape
- [x] `crates/runner/tests/webhook_contract.rs`
  - update or add one tracer-bullet test that still exercises the public `run` interface while the routing boundary changes underneath it
- [x] `crates/runner/tests/reconcile_contract.rs`
  - touch only if an existing public-behavior assertion needs to become the explicit “webhook request reaches real PostgreSQL state” proof

## TDD Execution Order

### Slice 1: Tracer Bullet For The Runner Image Artifact Contract

- [x] RED: add one failing default-lane test that builds the runner image, inspects its direct entrypoint, and fails because the current suite has no artifact-level runner image harness
- [x] GREEN: add the minimal `runner_image_artifact_harness` support and make the image build/entrypoint test pass
- [x] REFACTOR: keep runner-image docker helpers in one support boundary instead of cloning verify-image harness logic into multiple tests

### Slice 2: Prove The Scratch Runtime Filesystem Is Runtime-Only

- [x] RED: add one failing test that exports the built runner image filesystem and requires only the runner payload plus scratch-image metadata
- [x] GREEN: extend `RunnerDockerContract` with a minimal-runtime-filesystem assertion and make the new image test pass
- [x] REFACTOR: keep filesystem assertions centralized in `runner_docker_contract` so README, CI, and image tests share one runtime-only vocabulary

### Slice 3: Keep The Image Command Surface Runtime-Only

- [x] RED: add one failing image-level `docker run --rm <image> --help` test that requires the runtime-only subcommands and rejects removed setup/verify markers
- [x] GREEN: wire the artifact harness to capture help output and make the command-surface assertion pass
- [x] REFACTOR: remove any duplicated help-surface assertions between CLI and image tests by reusing the existing public-contract helpers where appropriate

### Slice 4: Flatten The Webhook Routing Table Boundary

- [x] RED: add one failing public-behavior test in `webhook_contract.rs` that still routes a request through `run` while locking in the expected mapping/table selection behavior
- [x] GREEN: replace the current `schema.table` string lookup path with a typed source-table key from payload parse through runtime-plan lookup
- [x] REFACTOR: delete `table_label()` string plumbing and keep display rendering only in error/reporting boundaries

### Slice 5: Keep The PostgreSQL Apply Path Explicit

- [x] RED: make one existing or new runtime contract test explicitly prove that a webhook request still reaches PostgreSQL state through the public `run` surface after the routing refactor
- [x] GREEN: adjust only the minimum runtime wiring needed to keep the apply path green
- [x] REFACTOR: do not split the runtime into separate host/container apply implementations; one public `run` path must remain the deep module

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, and `make test`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required default lane passes cleanly
- [x] REFACTOR: run one final `improve-code-boundaries` pass over the image-test helpers and webhook routing surface

## TDD Guardrails For Execution

- Start each slice with a failing public-behavior or artifact-level test before implementation.
- Do not add a second binary, shell entrypoint, or setup/verify runtime payload.
- Do not add a container-only runtime code path just to make image tests easier.
- Do not satisfy the boundary cleanup by pushing more `String` rendering deeper into runtime code.
- Do not swallow docker, TLS, config, or PostgreSQL errors. Keep typed failures explicit.
- Do not run `make test-long` unless the implementation genuinely changes the long-lane contract or the lane selection itself.

## Boundary Review Checklist

- [x] The runner image is still scratch and still starts the `runner` binary directly
- [x] The built runtime filesystem is minimal and runtime-only
- [x] The image help surface stays limited to `validate-config` and `run`
- [x] The public `run` path still applies webhook-driven state into PostgreSQL
- [x] No setup or verify responsibilities leak back into the runner image
- [x] No raw `schema.table` string lookup survives where a typed source-table key can carry the same meaning

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long` only if execution changes the long-lane contract or lane selection
- [x] One final `improve-code-boundaries` pass after all required lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-20-runner-scratch-image/01-task-build-the-runner-as-a-scratch-image-with-one-binary-that-applies-webhook-requests-to-postgresql_plans/2026-04-19-runner-scratch-image-apply-plan.md`

NOW EXECUTE
