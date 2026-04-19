# Plan: End-To-End External Network Fault Injection

## References

- Task: `.ralph/tasks/story-11-e2e-chaos/03-task-e2e-network-fault-injection-imposed-externally.md`
- Previous story-11 execution plans:
  - `.ralph/tasks/story-11-e2e-chaos/01-task-e2e-http-retry-chaos-imposed-externally_plans/2026-04-19-http-retry-chaos-e2e-plan.md`
  - `.ralph/tasks/story-11-e2e-chaos/02-task-e2e-receiver-crash-and-restart-recovery_plans/2026-04-19-receiver-crash-restart-recovery-plan.md`
- Existing long-lane scenarios:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Existing long-lane support:
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/webhook_chaos_gateway.rs`
- Design and test strategy:
  - `designs/crdb-to-postgres-cdc/07_test_strategy.md`
  - `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Investigation baseline for external retry behavior:
  - `designs/crdb-to-postgres-cdc/01_investigation_log.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown and `07_test_strategy.md` are treated as approval for the interface and behavior priorities in this turn.
- This task must stay on the real public path:
  - `runner run --config <path>`
  - `source-bootstrap render-bootstrap-script --config <path>`
  - `runner verify --config <path> --mapping <id> --source-url <url> [--allow-tls-mode-disable]`
- Network chaos must be imposed outside the runner. No in-binary failpoint, feature flag, alternate webhook route, or hidden retry hook is allowed.
- This task should prove transport instability, not merely application-level non-`200` behavior already covered by story-11 task 01.
- The cleanest deterministic transport fault is an external HTTPS gateway that accepts the Cockroach connection and then aborts the selected request path without returning an HTTP response.
- The gateway should inject the transport fault before forwarding the selected request upstream. That proves honest network retry behavior without depending on brittle "forward then sever the socket mid-response" timing.
- Because the first attempt never reaches the runner in this design, this task should prove retry/resume and post-recovery convergence, not duplicate-delivery idempotency. Duplicate-delivery behavior is already covered by the external HTTP `500` task.
- The scenario should include one later post-recovery source mutation after the faulted update converges so the test proves the pipeline resumes normal reconcile behavior rather than merely surviving a single retry.
- If the current gateway stack cannot abort a selected TLS/HTTP request deterministically from outside the runner without swallowing the fault behind an ordinary HTTP status, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - Cockroach retries after an externally imposed transport-level failure
  - the runner does not need any internal test hook or manual source-side rescue command
  - the faulted live update eventually reaches helper shadow state and the real destination table
  - the pipeline continues reconciling correctly for a later post-recovery update
  - `runner verify` still succeeds against the real migrated table only
- Lower-priority implementation concerns:
  - keep the long-lane test readable
  - keep transport-fault logic inside dedicated test support rather than inflating the generic harness

## Interface And Boundary Decisions

- Keep all product CLIs unchanged.
- Generalize the existing external gateway support so it owns typed externally imposed sink faults rather than only HTTP-500 overrides.
- Flatten the current wrong boundary in `crates/runner/tests/support/webhook_chaos_gateway.rs`:
  - today it mixes generic request observation with one hard-coded HTTP status fault shape
  - after this task it should expose a typed fault policy boundary that can express both HTTP response overrides and transport disconnects
- Keep `CdcE2eHarness` responsible for Docker lifecycle, config materialization, runner startup, SQL/query helpers, and polling.
- Keep the external gateway responsible for:
  - listening on its own HTTPS port
  - forwarding ordinary requests to the real runner
  - injecting one selected external sink fault
  - recording per-request outcomes so the test can assert retry/resume directly
- Keep scenario-specific customer operations in `default_bootstrap_harness.rs`.
- Prefer a typed fault API over stringly method sprawl. Example direction:
  - `ExternalSinkFault::HttpStatus { status: 500 }`
  - `ExternalSinkFault::AbortConnectionBeforeForward`
- The exact type names may change during execution, but the support boundary must stay typed and external.

## Public Contract To Establish

- One ignored long-lane test proves externally imposed network instability end to end:
  - baseline migration bootstraps through the real bootstrap script and real runner
  - a later source update is issued for the default customers scenario
  - the external HTTPS gateway aborts the first matching transport attempt without returning an HTTP response
  - Cockroach retries the same changefeed payload to the same external gateway
  - the gateway later forwards the retried request successfully to the real runner
  - helper shadow state and the real destination table converge to the correct updated value
  - a second post-recovery source update also converges without recreating the changefeed or runner
  - `runner verify` succeeds against the real migrated table
- The test must assert retry/resume concretely through gateway-observed fault and success outcomes for the same logical request fingerprint, not by inference from final table state alone.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - `crates/runner/tests/support/webhook_chaos_gateway.rs` currently models external chaos as an HTTP-500-specific special case, which puts the wrong abstraction at the gateway boundary for this task.
- Required cleanup during execution:
  - replace HTTP-500-only arming methods with a typed external sink fault API
  - keep gateway outcome recording inside the gateway module instead of scattering transport-fault assertions through `e2e_harness.rs`
  - keep customer-specific convenience helpers in `default_bootstrap_harness.rs`, not in the generic gateway
- Smells to avoid:
  - adding a second special-purpose gateway module for transport faults
  - adding ad hoc booleans like `drop_connection=true`
  - pushing gateway-specific state-machine details into the long-lane test body

## Files And Structure To Add Or Change

- [ ] `crates/runner/tests/support/webhook_chaos_gateway.rs`
  - generalize the gateway into typed external sink fault injection and request outcome capture
  - support one deterministic transport fault mode for this task
- [ ] `crates/runner/tests/support/e2e_harness.rs`
  - replace HTTP-500-specific gateway hooks with typed external fault helpers and retry observation helpers
  - keep runner/sink wiring readable
- [ ] `crates/runner/tests/support/default_bootstrap_harness.rs`
  - add scenario helpers for arming one external network fault on a customer update
  - add a concise post-recovery update flow if the test body becomes too procedural
- [ ] `crates/runner/tests/default_bootstrap_long_lane.rs`
  - add the ignored external-network-fault scenario
- [ ] Product code changes are not expected
  - only real bug fixes are allowed if the RED slices expose one
  - likely hotspots only if a real correctness bug appears:
    - `crates/runner/src/webhook_runtime/persistence.rs`
    - `crates/runner/src/reconcile_runtime/mod.rs`
    - `crates/runner/src/tracking_state.rs`

## TDD Execution Order

### Slice 1: Tracer Bullet For One External Transport Fault

- [ ] RED: add one ignored failing long-lane test that bootstraps the default customers migration, arms one external network fault for a live customer-email update, performs the update, and expects the destination row to converge after retry
- [ ] GREEN: implement only the minimum gateway and harness wiring needed to abort the first matching external request without an HTTP response and let the retry succeed
- [ ] REFACTOR: keep gateway fault policy behind typed support boundaries instead of embedding raw transport logic in the test

### Slice 2: Prove Retry/Resume Through Gateway-Observed Outcomes

- [ ] RED: strengthen the scenario to assert that the gateway recorded both:
  - one injected transport abort for the selected request fingerprint
  - one later successful forward for that same fingerprint
- [ ] GREEN: add the minimum typed gateway outcome recording needed for those assertions
- [ ] REFACTOR: keep fingerprinting and outcome formatting inside gateway support, not in the long-lane test

### Slice 3: Prove Real-State Catch-Up After Recovery

- [ ] RED: extend the scenario to assert helper shadow state and the real destination table both converge to the faulted update and then remain stable across an additional reconcile window
- [ ] GREEN: fix only the first real correctness bug exposed by those convergence assertions
- [ ] REFACTOR: keep customer snapshot helpers in `default_bootstrap_harness.rs`

### Slice 4: Prove Continuous Processing After The Transient Fault

- [ ] RED: extend the same scenario with a second post-recovery source update and assert the destination converges again without re-bootstrap, changefeed recreation, or runner restart
- [ ] GREEN: make only the minimal fix needed for the live pipeline to continue after the transient outage
- [ ] REFACTOR: avoid broad sleeps and prefer observable gateway state plus existing polling helpers

### Slice 5: Real Verify And No Binary Shortcut Regression

- [ ] RED: run `runner verify` after the post-recovery convergence and assert the output reports a matched verdict for the real migrated table only
- [ ] GREEN: keep verification at the CLI boundary; do not introduce any alternate verification path
- [ ] REFACTOR: confirm all transport-chaos logic lives only under `crates/runner/tests/support/` and no product file gained a test-only branch

### Slice 6: Full Repository Lanes And Final Boundary Review

- [ ] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [ ] GREEN: continue until every required lane passes cleanly
- [ ] REFACTOR: do one final `improve-code-boundaries` pass so the gateway support, generic harness, and customer scenario boundary each own one clear responsibility

## TDD Guardrails For Execution

- Every new behavior assertion must fail before code changes are added. If an assertion already passes, replace it with the next uncovered behavior.
- Do not replace a transport fault with an HTTP `500` and call it done. Task 01 already covers that lane.
- Do not fake webhook payloads, bypass the real HTTPS ingest path, or add hidden retry behavior inside the runner.
- Do not recreate the changefeed, restart the runner, or run extra source-side rescue commands for this scenario.
- Do not hide retry timing behind large sleeps. Prefer gateway outcome polling, helper-shadow polling, and destination polling.
- Do not swallow gateway, TLS, runner, Docker, bootstrap, or verify failures. Any such failure is a real test failure.

## Boundary Review Checklist

- [ ] External transport-fault injection is expressed as a typed gateway policy, not an HTTP-500-only special case
- [ ] The generic harness does not own raw gateway state-machine logic
- [ ] Retry/resume is asserted directly through gateway-recorded outcomes, not guessed from final table state alone
- [ ] Customer-specific assertions stay in `default_bootstrap_harness.rs`
- [ ] Product code contains no transport-fault-only test hooks
- [ ] No error path is swallowed or silently ignored

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long`
- [ ] One final `improve-code-boundaries` pass after all lanes are green
- [ ] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
