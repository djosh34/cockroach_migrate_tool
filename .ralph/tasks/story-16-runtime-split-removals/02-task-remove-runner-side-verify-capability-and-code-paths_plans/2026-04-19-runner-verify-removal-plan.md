# Plan: Remove Runner-Side Verify Capability And Code Paths

## References

- Task: `.ralph/tasks/story-16-runtime-split-removals/02-task-remove-runner-side-verify-capability-and-code-paths.md`
- Related prior plans:
  - `.ralph/tasks/story-16-runtime-split-removals/01-task-remove-runner-source-cockroach-access-and-config_plans/2026-04-19-runner-source-access-removal-plan.md`
  - `.ralph/tasks/story-16-runtime-split-removals/03-task-remove-bash-bootstrap-flows-and-script-based-source-setup_plans/2026-04-19-sql-only-source-setup-plan.md`
- Future follow-up task that must stay separate:
  - `.ralph/tasks/story-18-verify-http-image/07-task-route-all-correctness-tests-through-the-verify-http-image-only.md`
- Current runner public-contract tests:
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/readme_contract.rs`
  - `crates/runner/tests/long_lane.rs`
- Current runner E2E integrity and harness surface:
  - `crates/runner/tests/e2e_integrity_contract.rs`
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/runner_docker_contract.rs`
  - `crates/runner/tests/support/runner_image_harness.rs`
  - `crates/runner/tests/support/runner_container_process.rs`
- Current operator docs:
  - `README.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- Story-16 task 01 already removed the main runner-owned `verify`, `cutover-readiness`, `compare-schema`, and `render-helper-plan` command/config surface. Task 02 should finish the residual verify-specific runner boundary work instead of redoing that removal.
- The remaining risk is mostly in tests, harnesses, and contract enforcement:
  - scattered forbidden-marker assertions
  - brittle file-content scans across runner test files
  - image/runtime contract checks that still need one canonical "runner is destination-only" boundary
- This task must not absorb story-18 verify-image routing work. Until the verify HTTP image exists, runner tests may still assert migration behavior through destination state, tracking tables, and typed runtime audits. What must be forbidden now is runner-owned verification capability and any hidden test-only verify path inside the runner or its harnesses.
- No backwards compatibility is allowed. If any stale verify-shaped flag, config field, helper, or README wording survives, it should be removed rather than tolerated behind a deprecated alias.
- If execution reveals that any currently-supported runner scenario still depends on a real runner-side verify path to work, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- The runner public contract remains destination-only:
  - `validate-config --config <path>`
  - `render-postgres-setup --config <path> --output-dir <dir>`
  - `run --config <path>`
- Verification-removal enforcement should have one canonical owner in test support.
  - Add a dedicated support module for the runner public surface and forbidden verify markers.
  - Reuse that support from CLI, config, README, and image/container tests instead of open-coding the same `verify`/`--source-url`/`--cockroach-schema` checks everywhere.
- E2E harnesses should stay honest and runtime-shaped.
  - Allowed public assertions: destination snapshots, helper-table state, tracking progress, `RuntimeShapeAudit`, and `PostSetupSourceAudit`
  - Forbidden public assertions: `verify_*` helpers, `runner verify`, source-side correctness shortcuts, or fake bypass toggles
- Image/container tests should prove only runtime startup and destination config behavior.
  - No hidden entrypoint or image-level subcommand should reintroduce verify capability.

## Improve-Code-Boundaries Focus

- Primary smell: verify-removal enforcement is scattered across several tests as raw string lists and repeated negative assertions.
  - Flatten that into one support boundary that owns the allowed runner command set plus the forbidden verify surface.
- Secondary smell: `crates/runner/tests/e2e_integrity_contract.rs` currently reaches into individual source files and asserts on ad hoc substrings.
  - Keep enforcement, but move the rule ownership into one small integrity-support boundary instead of repeating file-name-specific grep logic in the test itself.
  - Prefer checking the public harness boundary and typed audit APIs over broad repo scans whenever the behavior can be observed that way.
- Be aggressive about deletion.
  - If a verify-shaped harness method, skip toggle, or duplicated forbidden-marker list still exists, delete it rather than wrapping it in one more helper.

## Public Contract To Establish

- `runner --help` and the runner image/container contract expose only destination runtime commands.
- `validate-config` output, config fixtures, and README contain no `verify` section, no `verify=...` summary field, and no removed source-only flags.
- Runner E2E support exposes no public `verify_*` helper, no `runner verify` shell-out path, and no fake `--skip-verify` or bypass marker.
- Honest runner integration tests continue to prove runtime behavior through destination state and typed audits only:
  - `RuntimeShapeAudit`
  - `PostSetupSourceAudit`
  - tracking progress
  - destination snapshots / helper-table convergence
- Any attempt to reintroduce the following through the runner contract fails loudly:
  - `verify`
  - `cutover-readiness`
  - `--source-url`
  - `--cockroach-schema`
  - `--allow-tls-mode-disable`
  - verify-shaped skip/bypass toggles

## Files And Structure To Add Or Change

- [ ] `crates/runner/tests/support/runner_public_contract.rs`
  - new support owner for allowed runner commands plus forbidden verify markers
- [ ] `crates/runner/tests/cli_contract.rs`
  - route verify-removal assertions through the shared public-contract support
- [ ] `crates/runner/tests/config_contract.rs`
  - keep destination-only validate output and legacy-verify rejection coverage, reusing the shared support boundary where it improves clarity
- [ ] `crates/runner/tests/readme_contract.rs`
  - keep README destination-only contract checks but deduplicate verify-removal marker ownership
- [ ] `crates/runner/tests/e2e_integrity_contract.rs`
  - replace scattered stringly enforcement with a smaller integrity-boundary audit
- [ ] `crates/runner/tests/support/e2e_integrity.rs`
  - own the typed integrity audit surface and any curated forbidden-marker audit helpers that truly belong there
- [ ] `crates/runner/tests/support/default_bootstrap_harness.rs`
  - remove or rename any leftover verify-shaped helper surface if execution finds it
- [ ] `crates/runner/tests/default_bootstrap_long_lane.rs`
  - keep the runtime-behavior assertions honest and explicit without verify helpers
- [ ] `crates/runner/tests/support/runner_image_harness.rs`
  - prove the built runner image still exposes runtime-only behavior
- [ ] `crates/runner/tests/support/runner_container_process.rs`
  - keep container-runtime assertions destination-only
- [ ] `README.md`
  - update only if any verify-shaped wording still leaks through the public runner contract
- [ ] `crates/runner/src/lib.rs`
  - touch only if execution finds a stale CLI/help/output path that still leaks verify surface

## TDD Execution Order

### Slice 1: Tracer Bullet For A Canonical Runner Public Contract

- [ ] RED: add one failing contract path that centralizes the allowed runner commands and forbidden verify markers, then use it from at least one CLI/help assertion
- [ ] GREEN: add the shared public-contract support module and rewire the first test to pass through it
- [ ] REFACTOR: delete duplicated forbidden-marker lists from the touched test(s)

### Slice 2: Destination-Only Config And README Surface

- [ ] RED: add one failing config or README contract that proves a verify-shaped marker can no longer leak into the public runner path
- [ ] GREEN: route config/README enforcement through the shared support boundary and remove the first stale wording or duplicated marker definition
- [ ] REFACTOR: keep verify-removal marker ownership in one place instead of spreading it across CLI, config, and README tests

### Slice 3: Remove Residual Verify-Shaped Harness Surface

- [ ] RED: add one failing integrity/harness contract that proves runner E2E support exposes no verify helper, no `runner verify` path, and no bypass toggle
- [ ] GREEN: delete or rename the first leftover verify-shaped harness surface and keep the behavioral test green through runtime-shaped audits only
- [ ] REFACTOR: shrink `e2e_integrity_contract.rs` so it audits the public harness boundary rather than open-coding broad file scans

### Slice 4: Enforce Runtime-Only Image And Container Behavior

- [ ] RED: add one failing image/container contract that proves the shipped runner image still supports only runtime startup and destination config validation
- [ ] GREEN: use `RunnerDockerContract`, `RunnerImageHarness`, and `RunnerContainerProcess` to keep the runtime-only contract explicit and fix the first stale assumption
- [ ] REFACTOR: keep container/image command assembly and runtime-only assertions co-located behind support helpers

### Slice 5: Repository Lanes

- [ ] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix the first failing lane at a time
- [ ] GREEN: continue until every required lane passes cleanly
- [ ] REFACTOR: do one final `improve-code-boundaries` pass to confirm verify-removal enforcement is not scattered or stringly

## TDD Guardrails For Execution

- Start every slice with a failing test. Do not refactor first.
- Keep tests focused on public behavior and public test-support boundaries, not private implementation details.
- Do not add a compatibility shim for removed verify commands, removed verify config, or removed source flags.
- Do not absorb story-18 verify-image routing. This task removes runner-owned verify capability; it does not invent a fake verify-image contract early.
- If a rule can be expressed through a typed audit or shared support boundary, prefer that over a raw file-content grep.
- If a curated source-file audit remains necessary to prove absence of a hidden path, keep that audit narrow and owned by one support module rather than scattered across tests.

## Boundary Review Checklist

- [ ] No runner CLI help or image/container contract exposes `verify`
- [ ] No runner CLI help or image/container contract exposes `cutover-readiness`
- [ ] No runner contract exposes `--source-url`, `--cockroach-schema`, or `--allow-tls-mode-disable`
- [ ] No `validate-config` output prints `verify=...`
- [ ] No runner config fixture or README teaches a `verify:` section
- [ ] No runner E2E support exposes `verify_*` helpers or verify bypass toggles
- [ ] Verify-removal enforcement is owned by one shared support boundary instead of duplicated string lists

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long`
- [ ] One final `improve-code-boundaries` pass after all lanes are green
- [ ] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

Plan path: `.ralph/tasks/story-16-runtime-split-removals/02-task-remove-runner-side-verify-capability-and-code-paths_plans/2026-04-19-runner-verify-removal-plan.md`

NOW EXECUTE
