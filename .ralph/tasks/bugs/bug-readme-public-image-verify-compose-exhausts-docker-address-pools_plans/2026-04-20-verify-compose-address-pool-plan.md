# Plan: Make Novice Published Compose Artifacts Deterministic And Non-Leaky

## References

- Task:
  - `.ralph/tasks/bugs/bug-readme-public-image-verify-compose-exhausts-docker-address-pools.md`
- Story that discovered the bug:
  - `.ralph/tasks/story-24-readme-only-novice-e2e/01-task-verify-readme-alone-enables-a-full-public-image-migration-with-zero-repo-access.md`
- Current failing contract and support code:
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/runner/tests/support/novice_registry_only_harness.rs`
- Published novice verify artifact:
  - `artifacts/compose/verify.compose.yml`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This bug is about Docker resource ownership in the novice verify test path, not about weakening the public README or verify-service security contract.
- No backwards compatibility is required.
  - We may change the test-support interface freely if it leads to a clearer lifecycle boundary.
  - We must not paper over the bug by telling operators to prune Docker globally or by silently ignoring cleanup failures.
- The execution validation lanes for this bug remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` is not required unless execution proves this bug changed an ultra-long lane boundary.
- If the first RED slice proves the published `verify.compose.yml` itself must change to own a different network contract for real users, this plan must switch back to `TO BE VERIFIED` and execution must stop immediately instead of forcing a harness-only fix.

## Execution Feedback That Invalidated The Prior Plan

- Reproduced the original failure with:
  - `cargo test -p runner --test novice_registry_only_contract copied_compose_contracts_work_from_a_repo_free_operator_workspace -- --exact --nocapture`
- The first recheck after landing `network_mode: bridge` for `verify.compose.yml` still failed before reaching verify startup.
  - `docker compose run setup-sql emit-postgres-grants` now dies creating `tmp..._default` with `all predefined address pools have been fully subnetted`
- The same public surface review shows `artifacts/compose/setup-sql.compose.yml` and `artifacts/compose/runner.compose.yml` also omit an explicit network contract.
- Observed many leaked `*_default` bridge networks on the daemon, which means the bug is not only "this harness sometimes forgets to tear down later" and not only "verify.compose.yml allocates a network".
- Conclusion:
  - A harness-only fix is insufficient.
  - A verify-only artifact fix is also insufficient.
  - The whole README/public-image compose surface needs an explicit network contract per artifact so repo-free flows stop depending on Compose allocating new project-scoped default networks.

## Revised Design Direction

- The public compose artifacts must stop requiring newly allocated project bridge networks for single-service flows.
- Verified artifact-level direction:
  - `artifacts/compose/setup-sql.compose.yml` should use `network_mode: none`
  - `artifacts/compose/runner.compose.yml` should use `network_mode: bridge`
  - `artifacts/compose/verify.compose.yml` should keep `network_mode: bridge`
  - the matching README snippets should mirror those contracts exactly
  - local verification confirmed `docker compose run` with `network_mode: none` succeeds without creating `<project>_default`
  - local verification confirmed `docker compose up` with `network_mode: bridge` succeeds without creating `<project>_default`
- After all artifact-level network contracts are corrected, the harness should still get an explicit cleanup boundary so teardown failures are no longer hidden in `Drop`.

## Current State Summary

- `copied_compose_contracts_work_from_a_repo_free_operator_workspace` starts the repo-free verify Compose flow through `NoviceRegistryOnlyHarness::start_verify_compose_runtime`.
- `start_verify_compose_runtime` generates a unique Compose project name and runs:
  - `docker compose -p <project> -f verify.compose.yml up -d verify`
- Because `verify.compose.yml` does not declare a custom network, Compose creates a fresh project-scoped default bridge network on each run.
- `RunningVerifyCompose` currently mixes three concerns:
  - startup
  - readiness polling
  - teardown
- `Drop for RunningVerifyCompose` runs `docker compose down --remove-orphans` and discards the command result entirely.
  - This violates the repo rule against swallowing errors.
  - If cleanup fails, the test leaves Docker resources behind without any direct signal.
- The observed runtime failure is consistent with leaked per-project bridge networks accumulating until Docker reports:
  - `all predefined address pools have been fully subnetted`

## Interface And Boundary Decisions

- The published Compose YAML owns the startup-network contract.
  - The first implementation target is the public artifact and README contract, not the harness lifecycle boundary.
- Preserve the public listener access shape.
  - Keep the published `ports` mapping contract.
  - Do not switch to `network_mode: host`; that would change the public port/bind-address contract unnecessarily.
- Make the network choice explicit instead of ambient.
  - `network_mode: bridge` is the new public contract for this single-service artifact.
- Replace hidden cleanup in `Drop` with an explicit, verified lifecycle boundary.
  - A caller should be able to start the verify Compose runtime, wait until it is running, then shut it down and know whether cleanup actually succeeded.
- Separate compose-project ownership from service-readiness polling.
  - The current `RunningVerifyCompose` type knows too much and hides failure too far away from the caller.
  - Execution should split or reshape this so one boundary owns Docker resources and one boundary checks observable runtime behavior.
- Keep failure evidence local and typed enough to debug quickly.
  - Startup failures should still show compose logs.
  - Cleanup failures should fail with the exact `docker compose down` status and output, not disappear into a best-effort `Drop`.

## Improve-Code-Boundaries Focus

- Primary smell: mixed responsibilities.
  - `RunningVerifyCompose` both models a running service and secretly performs cleanup side effects.
- Secondary smell: wrong-place ownership.
  - The test body thinks it is verifying public compose behavior, but the actual resource lifecycle is hidden in a drop implementation inside support code.
- Secondary smell: swallowed errors.
  - The compose teardown boundary must stop ignoring command failures entirely.
- Execution should end with one clean owner for:
  - compose startup
  - network/resource existence checks
  - explicit teardown

## Public Contract After Execution

- The novice registry-only verify compose flow must be repeatable without depending on a pristine Docker daemon address-pool state.
- The shipped README/public-image compose artifacts must each declare an explicit network contract instead of inheriting Compose's project default network allocation behavior.
- If the verify compose runtime cannot tear down its owned resources, the test must fail directly with actionable cleanup evidence.
- The published novice verify flow must keep working from a repo-free copied workspace.
- The fix must not rely on global Docker cleanup outside the resources owned by this test.

## Files And Structure To Change

- [ ] `crates/runner/tests/novice_registry_only_contract.rs`
  - add the first deterministic RED contract around the explicit public compose network contracts
  - then add the cleanup-ownership contract once the artifact startup behavior is fixed
  - keep the broader repo-free compose contract as the end-to-end proof
- [ ] `crates/runner/tests/support/novice_registry_only_harness.rs`
  - refactor the verify compose helper so cleanup is explicit and verified
  - remove swallowed teardown errors
  - separate lifecycle ownership from readiness polling if that deepens the boundary
- [x] `artifacts/compose/setup-sql.compose.yml`
  - make the one-shot setup surface explicit with `network_mode: none` instead of inheriting a project default network
- [x] `artifacts/compose/runner.compose.yml`
  - make the runner listener surface explicit with `network_mode: bridge` instead of inheriting a project default network
- [x] `artifacts/compose/verify.compose.yml`
  - keep the single-service network contract explicit with `network_mode: bridge`
  - preserve the existing published port mapping contract
- [x] `README.md`
  - keep the pasted compose examples aligned with the published artifact contracts

## TDD Execution Order

### Slice 1: Re-Plan The Public Compose Network Contracts

- [x] RED:
  - add the smallest public behavior tests that prove each shipped compose artifact declares its explicit runtime network contract
  - assert the long-lived single-service listener artifacts use `network_mode: bridge`
  - assert the one-shot setup artifact uses `network_mode: none` so the local render-only command cannot allocate a project bridge network
  - keep asserting that the public artifact stays registry-only and repo-free
- [x] GREEN:
  - update the shipped compose artifacts
  - update the README snippets so the public examples match the shipped artifacts exactly
- [x] REFACTOR:
  - keep the public compose artifacts honest and minimal
  - avoid adding named custom networks or extra YAML surface where a simpler explicit contract works

### Slice 2: Explicit Harness Teardown Boundary

- [x] RED:
  - add a focused integration contract proving the harness reports verify compose teardown failures instead of hiding them in `Drop`
- [x] GREEN:
  - replace hidden cleanup with explicit verified shutdown once the artifact-level startup contract is correct
- [x] REFACTOR:
  - split lifecycle ownership from readiness polling if the helper still mixes concerns

### Slice 3: Re-Verify The Original Failing Public Contract

- [x] Re-run:
  - `cargo test -p runner --test novice_registry_only_contract copied_compose_contracts_work_from_a_repo_free_operator_workspace -- --exact --nocapture`
- [ ] If the original bug still reproduces, capture the next real failure and write the next RED test before fixing it
- [ ] If the design assumption changes and the fix now clearly belongs in the published Compose artifact, switch this plan back to `TO BE VERIFIED` and stop immediately

### Slice 4: Repository Validation Lanes

- [x] Run `make check`
- [x] Run `make lint`
- [x] Run `make test`
- [ ] Do not run `make test-long` unless execution proves this bug changed an ultra-long lane boundary
- [x] Do one final `improve-code-boundaries` pass so the verify compose lifecycle has one clear owner and no swallowed errors remain

## Expected Boundary Outcome

- The public README compose artifacts no longer depend on Compose creating a brand-new bridge subnet on each run.
- The long-lived listener artifacts make that guarantee explicit by using `network_mode: bridge`.
- The one-shot setup artifact uses `network_mode: none`, so it cannot allocate an unnecessary project-scoped bridge network.
- The novice verify compose support code stops leaking Docker-network cleanup failures behind `Drop`.
- The compose lifecycle becomes explicitly owned and testable.
- The repo-free novice verify contract becomes repeatable instead of depending on ambient Docker address-pool luck.

Plan path: `.ralph/tasks/bugs/bug-readme-public-image-verify-compose-exhausts-docker-address-pools_plans/2026-04-20-verify-compose-address-pool-plan.md`

NOW EXECUTE
