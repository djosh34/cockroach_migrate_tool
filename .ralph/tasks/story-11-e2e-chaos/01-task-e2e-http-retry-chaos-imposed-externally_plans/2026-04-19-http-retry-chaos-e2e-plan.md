# Plan: End-To-End HTTP Retry Chaos Imposed Externally

## References

- Task: `.ralph/tasks/story-11-e2e-chaos/01-task-e2e-http-retry-chaos-imposed-externally.md`
- Previous baseline E2E plan: `.ralph/tasks/story-10-e2e-baseline/01-task-e2e-default-database-bootstrap-from-scratch_plans/2026-04-19-default-bootstrap-e2e-plan.md`
- Previous delete E2E plan: `.ralph/tasks/story-10-e2e-baseline/03-task-e2e-delete-propagation-through-shadow-and-real-tables_plans/2026-04-19-delete-propagation-e2e-plan.md`
- Existing long-lane scenarios:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Existing long-lane support:
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
- Existing duplicate-delivery contract:
  - `crates/runner/tests/webhook_contract.rs`
- Current webhook runtime:
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/routing.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
- Test strategy:
  - `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- This task is an ignored real E2E scenario. It must keep the shipped public flow:
  - `runner run --config <path>`
  - `source-bootstrap render-bootstrap-script --config <path>`
  - `runner verify --config <path> --mapping <id> --source-url <url> [--allow-tls-mode-disable]`
- HTTP failure must be imposed outside the runner. The runner must not gain a test flag, hidden branch, or internal shortcut that fakes retries.
- The key behavior is not just "eventual success after some 500s". The test must force at least one duplicate delivery of the same real webhook batch so helper-shadow idempotency is proven under the shipped persistence path.
- The cleanest honest mechanism is a test-side HTTPS gateway in front of the runner:
  - Cockroach changefeed sends to the gateway URL.
  - The gateway forwards to the real runner.
  - The gateway can intentionally answer `500` to Cockroach on selected attempts even after the runner has already returned `200`.
- The scenario should use a small live update after bootstrap rather than re-testing the entire initial-scan bootstrap path. Story 10 already proved the baseline happy path; story 11 task 01 should extend that path with externally imposed response chaos.
- If the first RED slice proves Cockroach retry behavior cannot be observed safely through an external response gate, or that the current long-lane support cannot separate the public sink endpoint from the runner bind endpoint cleanly, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- The task markdown and `07_test_strategy.md` are treated as approval for the public interface and test priorities for this turn.
- Highest-priority behaviors to prove:
  - Cockroach retries after an externally imposed HTTP `500`
  - the same row batch can be delivered more than once through the real HTTPS ingest path
  - helper shadow persistence stays idempotent under that duplicate delivery
  - the real destination table converges to the correct final state
  - the binary contains no test-only shortcut logic for the scenario

## Interface And Boundary Decisions

- Keep all product CLIs unchanged.
- Split two roles that are currently conflated inside `crates/runner/tests/support/e2e_harness.rs`:
  - runner listener endpoint
  - public webhook sink endpoint exposed to Cockroach
- Introduce one explicit test-support boundary for external HTTP chaos:
  - `WebhookChaosGateway` or equivalently named support module under `crates/runner/tests/support/`
  - responsibilities:
    - listen on its own HTTPS port
    - proxy matching requests to the real runner
    - apply a typed failure policy to the HTTP response seen by Cockroach
    - record attempt counts and request fingerprints for assertions
- Keep the generic E2E harness responsible for Docker lifecycle, temp config generation, runner startup, query helpers, and source mutations.
- Keep the chaos gateway responsible for externally imposed response behavior only. It must not own helper-table SQL or destination assertions.
- Deepen the default customers scenario support so the long-lane test reads as behavior, while the gateway module owns proxying and retry-attempt observation.

## Public Contract To Establish

- One ignored long-lane test proves externally imposed HTTP response chaos end to end:
  - baseline migration bootstraps through the real `source-bootstrap` script and real `runner run`
  - a later source update produces a real changefeed webhook batch
  - the external HTTPS gateway forwards that batch to the runner but intentionally returns `500` to Cockroach on the first selected attempt
  - Cockroach retries the same batch against the gateway
  - the runner sees duplicate delivery through its normal ingest endpoint
  - helper shadow tables remain correct and non-duplicated
  - the real destination tables converge to the correct final state
  - `runner verify` succeeds against the real migrated tables
- The test must assert duplicate delivery concretely through gateway-observed attempts or captured request fingerprints, not by inference from final table state alone.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - `crates/runner/tests/support/e2e_harness.rs` currently assumes the runner bind port is also the public sink endpoint. That makes externally imposed HTTP response chaos awkward and pushes unrelated responsibilities into one support module.
- Required cleanup during execution:
  - separate runner endpoint wiring from sink endpoint wiring
  - move proxy/chaos logic into a dedicated support module instead of bloating the generic harness
  - keep scenario-specific customer assertions inside `default_bootstrap_harness.rs` or a more honestly named customer-scenario support module
- Preferred cleanup shape:
  - generic harness exposes "where the real runner listens"
  - chaos gateway exposes "where Cockroach should send changefeed requests"
  - long-lane test composes the two without embedding proxy state machines inline
- Do not add ad hoc stringly flags like "chaos=true" into runner config generation if the same behavior can be expressed through typed support objects.

## Files And Structure To Add Or Change

- [x] `crates/runner/tests/support/e2e_harness.rs`
  - separate runner listener address from source-bootstrap webhook sink address
  - add typed hooks so a scenario can point Cockroach at an external HTTPS gateway instead of directly at the runner
- [x] `crates/runner/tests/support/default_bootstrap_harness.rs`
  - deepen the customers scenario support with helpers for live customer updates, helper-shadow snapshots, and destination snapshots used by the retry-chaos test
- [x] `crates/runner/tests/support/webhook_chaos_gateway.rs`
  - new support module for the external HTTPS proxy/failure policy and request-attempt capture
- [x] `crates/runner/tests/default_bootstrap_long_lane.rs`
  - add the ignored external-HTTP-retry-chaos scenario and keep the existing baseline/delete scenarios readable
- [x] Product code changes are not expected
  - only real bug fixes are allowed if the RED slices expose them
  - likely hotspots only if a real correctness gap appears:
    - `crates/runner/src/webhook_runtime/persistence.rs`
    - `crates/runner/src/reconcile_runtime/mod.rs`
    - `crates/runner/src/tracking_state.rs`
- [x] No source-bootstrap CLI shape change is expected
  - the existing rendered `webhook.base_url` should simply point at the external gateway URL

## TDD Execution Order

### Slice 1: Tracer Bullet For Externally Imposed HTTP Retry

- [x] RED: add one ignored failing long-lane test that bootstraps the default customers migration through a gateway URL, performs a live source update, injects one external HTTP `500`, and waits for the destination row to converge to the updated value
- [x] GREEN: implement the minimum gateway and harness wiring needed for that scenario to pass through the real public flow
- [x] REFACTOR: keep sink-endpoint wiring out of the long-lane test body and behind typed support boundaries

### Slice 2: Prove Duplicate Delivery Reaches The Runner

- [x] RED: extend the scenario so the gateway forwards the first matching row batch to the runner, then still returns `500` to Cockroach, forcing a duplicate retry of the same payload
- [x] GREEN: add only the minimum gateway behavior needed to override the outward response after a successful upstream forward
- [x] REFACTOR: represent the failure policy with typed match criteria and attempt counters rather than raw string toggles

### Slice 3: Prove Helper Shadow Idempotency Under Duplicate Delivery

- [x] RED: extend the scenario to assert both:
  - the gateway observed at least two deliveries of the same logical batch
  - helper shadow state contains exactly the correct updated customer row once, with no duplicate net state
- [x] GREEN: fix only the first real correctness bug exposed by that duplicate-delivery assertion
- [x] REFACTOR: keep helper-shadow snapshot/assertion helpers inside the customers support boundary rather than reassembling SQL in the test

### Slice 4: Prove Real-Table Convergence After Retry

- [x] RED: extend the scenario to assert the real destination table converges to the correct updated row after the duplicate-delivery replay, then stays stable across an additional reconcile window
- [x] GREEN: make only the minimal fix needed for the real pipeline to converge
- [x] REFACTOR: reuse existing generic polling helpers and avoid broad sleeps that hide ordering bugs

### Slice 5: Real Verify And No Binary Shortcut Regression

- [x] RED: run `runner verify` after convergence and assert the output reports a matched verdict for the real migrated table only
- [x] GREEN: keep verification at the CLI boundary; do not introduce a second verification path
- [x] REFACTOR: confirm all scenario-specific chaos code lives only under `crates/runner/tests/support/` and no product file gained a test-only branch

### Slice 6: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so the generic harness, the gateway support, and the customers scenario boundary each own one clear responsibility

## TDD Guardrails For Execution

- Every new behavior assertion must fail before code changes are added. If an assertion already passes, replace it with the next uncovered behavior.
- Do not fake webhook payloads, bypass the real HTTPS ingest endpoint, or inject retries from inside the runner.
- Do not mutate Cockroach manually after CDC setup except for the planned source data change that drives the live update under test.
- Do not hide retry timing behind large sleeps. Prefer observable request-attempt counters and polling helpers.
- Do not silently downgrade gateway, runner, Docker, bootstrap, or verify failures. Any such failure is a real test failure.
- If the gateway design needs a second ad hoc config file or stringly shell wrapper to express per-request response policy, fix the support boundary instead of layering more glue.

## Boundary Review Checklist

- [x] The runner bind address and the Cockroach sink address are no longer conflated in generic support
- [x] External HTTP chaos lives in dedicated test support, not in product code
- [x] Duplicate delivery is asserted directly through captured attempts, not guessed from final table state
- [x] Helper-shadow assertions stay in the customer-scenario support boundary
- [x] `runner verify` remains at the public CLI boundary
- [x] No error path is swallowed or silently ignored

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
