# Plan: Remove Host-Process Runner Webhook Port TOCTOU From Contract Tests

## References

- Task: `.ralph/tasks/bugs/bug-runner-test-port-selection-flakes-webhook-bind.md`
- Current flaky host-process contract files:
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
  - `crates/runner/tests/webhook_contract.rs`
- Existing but underused shared test support:
  - `crates/runner/tests/support/mod.rs`
  - `crates/runner/tests/support/runner_process.rs`
- Runtime webhook bind boundary:
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This bug is a test-harness ownership bug, not a production ingress-policy bug.
  - The runner is failing because tests choose a port first, release it, and only later ask the runner to bind it.
  - The production webhook runtime should keep owning the actual socket bind.
- No backwards compatibility is required.
  - It is acceptable to change the host-process contract harness API if that removes duplicate port and URL plumbing.
  - Existing tests should be rewritten to the cleaner helper instead of preserving the current preselected-port shape.
- The task markdown plus this plan are the approval for the interface direction in this turn.
- Required validation lanes for execution remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` stays out of scope unless execution proves this bug also affects an ultra-long lane that is part of the task boundary.
- If the first RED slice proves the runner cannot expose its real bound webhook address cleanly without test-only backdoors, brittle stderr string scraping scattered across files, or a larger runtime contract redesign, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- The flaky failure came from a host-process runner contract test in `crates/runner/tests/reconcile_contract.rs`.
- The same TOCTOU pattern exists in multiple runner contract files:
  - `pick_unused_port()` binds `127.0.0.1:0`, reads the chosen port, closes the listener, then writes that port into runner config.
  - Tests later construct `https://localhost:{bind_port}/healthz` from the stale preselected port.
- This logic is duplicated across the biggest host-process contract files:
  - `bootstrap_contract.rs`
  - `reconcile_contract.rs`
  - `webhook_contract.rs`
- There is also a boundary smell:
  - those files each own their own `RunnerProcess` implementation even though shared runner test support already exists
  - port selection, process launch, log handling, and health URL construction are spread across multiple files instead of one deep support module
- The cleanest ownership model is to stop preselecting the webhook port entirely for host-process tests:
  - write runner config with `127.0.0.1:0`
  - let the runner bind its own socket
  - discover the real bound address after startup through one shared support boundary

## Improve-Code-Boundaries Focus

- Primary smell: test files own runtime bootstrap details they should not own.
  - choosing webhook ports
  - constructing health and metrics URLs
  - duplicating child-process log plumbing
- Secondary smell: shared support exists but the largest host-process contract files bypass it.
- Preferred cleanup direction:
  - deepen shared runner test support so it owns child-process launch, startup event reading, and discovered webhook base URL
  - delete duplicated `RunnerProcess` helpers from the large contract files
  - remove the runner-webhook `pick_unused_port()` pattern from host-process contract coverage entirely
- Do not smear the fix across ad hoc per-test retries or sleeps.
  - the port-ownership contract should become correct, not merely less flaky

## Public Contract After Execution

- Host-process runner contract tests must no longer require a caller-chosen webhook port.
- The runner must be able to start with `webhook.bind_addr: 127.0.0.1:0` in host-process contract coverage.
- Shared test support must expose the real HTTPS base URL after the runner binds.
- Contract tests must use that discovered URL for:
  - `/healthz`
  - `/metrics`
  - webhook ingest endpoints
- A deterministic RED test must prove the old TOCTOU shape is a bug.
  - occupying the previously chosen port between config write and process start must not break the safe launch path after the fix
- The original reconcile/webhook/bootstrap contract coverage must keep passing with the new helper.

## Expected Code Shape

- `crates/runner/src/lib.rs`
  - thread the runtime event sink into webhook startup if the cleanest way to expose the real bound address is a structured runtime event
- `crates/runner/src/webhook_runtime/mod.rs`
  - after successful bind, expose the actual listener address through the normal runtime logging/event boundary
  - do not change webhook serving semantics beyond surfacing the already-owned bound address
- `crates/runner/tests/support/mod.rs`
  - export the shared runner-process support used by host-process contract files
- `crates/runner/tests/support/runner_process.rs`
  - deepen it so it can start the runner, wait for the startup/bound-address event, and return discovered HTTPS URLs
  - keep failure logs readable when startup fails before readiness
- `crates/runner/tests/bootstrap_contract.rs`
- `crates/runner/tests/reconcile_contract.rs`
- `crates/runner/tests/webhook_contract.rs`
  - stop defining their own runner-process helper
  - stop preselecting webhook ports for host-process runner startup
  - route all health/metrics/ingest calls through shared discovered URLs
- Avoid:
  - test-only environment variables or sidecar files just to leak the port out
  - global retry loops that hide the race instead of removing it
  - preserving duplicate local helpers for “convenience”

## Type And Boundary Decisions

- Preferred test-support boundary:
  - one shared host-process runner helper owns:
    - start mode
    - structured startup log parsing
    - discovered bind address
    - derived HTTPS URLs
- Preferred runtime signal:
  - emit one structured event after webhook bind succeeds with the actual bound socket address
- Preferred config shape in host-process tests:
  - `webhook.bind_addr: 127.0.0.1:0`
- Keep the runner runtime authoritative for port ownership.
  - tests should discover the bound address after startup instead of “reserving” it before startup
- Do not add:
  - a new public config field for reporting startup ports
  - a separate fake listener handoff path for tests
  - multiple competing runner-process support helpers

## TDD Execution Order

### Slice 1: Tracer Bullet For The Race

- [x] RED: add one failing host-process integration test that honestly captures the TOCTOU bug by occupying the formerly preselected webhook port between config creation and runner start, while the desired contract is that the safe shared launch path still reaches healthz
- [x] GREEN: make the smallest coherent change that lets the runner own the ephemeral port and lets test support discover the real bound address after startup
- [x] REFACTOR: move the startup/bound-address knowledge into shared support instead of leaving parsing or URL construction in the test file

### Slice 2: Migrate The Reconcile Contract Surface

- [x] RED: rerun the focused reconcile contract lane and let the next failing assumption show where it still depends on the stale `bind_port` shape
- [x] GREEN: convert reconcile host-process tests to the shared discovered-URL helper
- [x] REFACTOR: remove now-dead `pick_unused_port()` and local `RunnerProcess` code from `reconcile_contract.rs`

### Slice 3: Migrate Webhook And Bootstrap Contract Surfaces

- [x] RED: rerun focused webhook/bootstrap contract lanes one at a time and let each next failure expose any remaining hard-coded preselected-port assumption
- [x] GREEN: convert those files to the shared discovered-URL helper without reintroducing duplicate launch logic
- [x] REFACTOR: remove duplicate local runner-process helpers and collapse common path construction into shared support

### Slice 4: Manual Verification And Boundary Audit

- [x] Manually verify the original bug no longer holds by checking host-process runner contract coverage for any remaining webhook `pick_unused_port()` dependency
- [x] If another honest host-process runner path still depends on the stale pattern, add one new RED test and repeat the cycle before moving on
- [x] Run one final `improve-code-boundaries` sweep so the host-process runner boundary is deeper than before, not muddier

### Slice 5: Repository Validation

- [x] Run `make check`
- [x] Run `make lint`
- [x] Run `make test`
- [ ] `make test-long` remains out of scope unless execution proves this bug changed an ultra-long lane boundary

## Expected Boundary Outcome

- The runner owns webhook port selection in host-process contract coverage.
- Tests discover the real bind address instead of guessing it.
- One shared support helper replaces duplicated runner launch code in the large contract files.
- The flaky webhook bind failure should disappear because the TOCTOU port handoff no longer exists on that path.

Plan path: `.ralph/tasks/bugs/bug-runner-test-port-selection-flakes-webhook-bind_plans/2026-04-20-runner-webhook-port-ownership-plan.md`

NOW EXECUTE
