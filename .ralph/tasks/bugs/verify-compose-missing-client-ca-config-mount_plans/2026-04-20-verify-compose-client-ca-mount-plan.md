# Plan: Restore The Verify Compose Client CA Mount Contract

## References

- Task:
  - `.ralph/tasks/bugs/verify-compose-missing-client-ca-config-mount.md`
- Related broader novice-user contract:
  - `.ralph/tasks/story-25-verify-novice-user-registry-only/01-task-verify-registry-only-novice-user-can-complete-the-supported-flow.md`
  - `.ralph/tasks/story-25-verify-novice-user-registry-only/01-task-verify-registry-only-novice-user-can-complete-the-supported-flow_plans/2026-04-20-registry-only-novice-flow-plan.md`
- Related Compose publication contract:
  - `.ralph/tasks/story-21-github-workflows-image-publish/04-task-publish-separate-docker-compose-artifacts-for-runner-verify-and-sql-images_plans/2026-04-20-compose-artifacts-plan.md`
- Current failing public boundary:
  - `artifacts/compose/verify.compose.yml`
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/runner/tests/support/novice_registry_only_harness.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This is a public-contract regression in the shipped verify Compose artifact, not a test-only problem.
- No backwards compatibility is required.
  - If the verify service config requires `listener.tls.client_auth.client_ca_path`, the published Compose artifact must mount that file.
  - We must not weaken listener mTLS or change the novice config shape just to make the current artifact pass.
- The honest verification target remains the repo-free novice operator workspace exercised by Docker Compose.
- The required repository validation lanes for execution remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` is not required unless execution proves this bug changed an ultra-long lane boundary.
- If the first RED slice proves the client CA should not be mounted after all, or that the published verify Compose artifact cannot honestly own the verify listener certificate set, this plan must stay `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- The novice harness writes `config/verify-service.yml` with:
  - `listener.transport.mode: https`
  - `listener.tls.client_auth.mode: mtls`
  - `listener.tls.client_auth.client_ca_path: /config/certs/client-ca.crt`
- The novice harness also writes `config/certs/client-ca.crt` into the operator workspace.
- The shipped `artifacts/compose/verify.compose.yml` mounts:
  - verify service config
  - source CA
  - source client cert/key
  - destination CA
  - server cert/key
- The shipped artifact does not mount `client-ca.crt` into `/config/certs/client-ca.crt`.
- The existing public novice contract therefore fails at runtime with:
  - `open /config/certs/client-ca.crt: no such file or directory`

## Interface And Boundary Decisions

- Fix the shipped Compose artifact instead of mutating the novice config to avoid mTLS.
  - The public contract is the copied `verify.compose.yml`.
  - The test harness is only surfacing the broken artifact honestly.
- Keep the verify listener certificate set owned by the verify Compose contract.
  - The artifact must enumerate every file the mounted `verify-service.yml` needs at startup.
  - Do not spread a second source of truth for "required verify mounts" across unrelated helpers.
- Add or tighten contract coverage at the artifact boundary so this class of omission fails before a vague runtime log chase.
  - The runtime contract still matters.
  - But the boundary cleanup should make the missing mount obvious and local to the published artifact.

## Improve-Code-Boundaries Focus

- Primary smell: wrong-place ownership of the verify startup contract.
  - Today the required file list is implied partly by the generated `verify-service.yml` and partly by a hand-maintained Compose YAML.
  - Execution should make the Compose artifact explicitly satisfy the verify-service file contract and add a direct contract assertion around that published artifact.
- Secondary smell: muddy failure distance.
  - The current failure appears only after `docker compose up`, which is correct but too far from the actual broken boundary.
  - Execution should keep the end-to-end runtime proof while also adding a tighter artifact-level assertion if needed, so future regressions fail where the public artifact is defined.

## Public Contract After Execution

- A copied `verify.compose.yml` from `artifacts/compose/` must be sufficient for the documented novice verify startup when the operator provides:
  - `config/verify-service.yml`
  - `config/certs/source-ca.crt`
  - `config/certs/source-client.crt`
  - `config/certs/source-client.key`
  - `config/certs/destination-ca.crt`
  - `config/certs/client-ca.crt`
  - `config/certs/server.crt`
  - `config/certs/server.key`
- The novice registry-only contract must no longer fail with a missing `/config/certs/client-ca.crt`.
- The fix must live in the published artifact boundary, not in test-only bypasses or weakened listener security.

## Files And Structure To Change

- [ ] `artifacts/compose/verify.compose.yml`
  - add the missing client CA config entry and service mount target
- [ ] `crates/runner/tests/novice_registry_only_contract.rs`
  - keep or sharpen the public failing contract so the regression is captured explicitly
- [ ] `crates/runner/tests/support/novice_registry_only_harness.rs`
  - only adjust if the artifact-boundary assertion needs a cleaner helper or clearer failure message
- [ ] any focused artifact-contract helper/test added during execution
  - only if it reduces duplication and keeps mount ownership in one place

## TDD Execution Order

### Slice 1: Existing Red Runtime Contract

- [ ] RED:
  - use the existing failing public contract:
    - `cargo test -p runner --test novice_registry_only_contract copied_compose_contracts_work_from_a_repo_free_operator_workspace -- --exact`
  - treat that failing novice runtime as the first RED slice rather than inventing a second overlapping integration test
- [ ] GREEN:
  - make the smallest product change in `artifacts/compose/verify.compose.yml` that mounts `client-ca.crt` at `/config/certs/client-ca.crt`
- [ ] REFACTOR:
  - keep the fix in the public Compose artifact only
  - do not change the novice verify config to stop requiring mTLS

### Slice 2: Tighten The Artifact Boundary

- [ ] RED:
  - if the runtime slice goes green, add one focused contract assertion that the published verify Compose artifact includes the client CA config and target path required by the verify listener config
- [ ] GREEN:
  - keep the artifact contract green with the real published YAML, not a duplicated ad hoc string list in multiple files
- [ ] REFACTOR:
  - if a helper is needed, make it the one typed place that describes required verify Compose config targets for tests

### Slice 3: Manual Bug Re-Verification

- [ ] Re-run the novice registry-only verify flow after the fix
- [ ] If a different runtime failure appears, create the next RED test for that newly exposed public bug before fixing it
- [ ] If the design no longer looks correct, switch this plan back to `TO BE VERIFIED` immediately and stop

### Slice 4: Repository Validation Lanes

- [ ] Run `make check`
- [ ] Run `make lint`
- [ ] Run `make test`
- [ ] Do not run `make test-long` unless execution proves this bug changed an ultra-long lane boundary
- [ ] Do one final `improve-code-boundaries` pass so the verify Compose mount contract is owned cleanly and not duplicated across scattered helpers

## Expected Boundary Outcome

- The published verify Compose artifact once again matches the verify-service listener mTLS contract.
- The repo-free novice verify flow stays honest: it succeeds because the shipped artifact is correct, not because tests special-case around it.
- Future regressions around required verify cert mounts fail closer to the artifact boundary instead of only surfacing as distant container startup errors.

Plan path: `.ralph/tasks/bugs/verify-compose-missing-client-ca-config-mount_plans/2026-04-20-verify-compose-client-ca-mount-plan.md`

NOW EXECUTE
