# Plan: End-To-End Receiver Crash And Restart Recovery

## References

- Task: `.ralph/tasks/story-11-e2e-chaos/02-task-e2e-receiver-crash-and-restart-recovery.md`
- Previous story-11 execution plan:
  - `.ralph/tasks/story-11-e2e-chaos/01-task-e2e-http-retry-chaos-imposed-externally_plans/2026-04-19-http-retry-chaos-e2e-plan.md`
- Existing long-lane scenarios:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Existing long-lane support:
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/webhook_chaos_gateway.rs`
- Runtime surfaces involved in restartability:
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/tracking_state.rs`
- Design and test strategy:
  - `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown and `07_test_strategy.md` are treated as approval for the interface and behavior priorities in this turn.
- This task must stay on the real public path:
  - `runner run --config <path>`
  - `source-bootstrap render-bootstrap-script --config <path>`
  - `runner verify --config <path> --mapping <id> --source-url <url> [--allow-tls-mode-disable]`
- No test-only product branch, env var, failpoint, or debug endpoint may be added to force a crash window.
- The honest place to control crashes is test support that owns the spawned `runner` process and kills or restarts it externally.
- The honest place to observe progress is durable destination state:
  - helper shadow table contents
  - `_cockroach_migration_tool.stream_state`
  - `_cockroach_migration_tool.table_sync_state`
  - real destination tables
- The helper-persistence-before-reconcile window can be forced with a long reconcile interval and assertions that helper state advanced while real tables did not.
- The mid-reconcile crash window can be forced by holding a real PostgreSQL lock from test support so reconcile blocks inside a real transaction, then killing the runner process while that reconcile pass is stuck.
- If the lock-based mid-reconcile window cannot be made deterministic without product-only hooks, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - the runner can restart after helper shadow persistence but before reconcile and still converge correctly
  - the runner can restart while a real reconcile pass is in progress and still converge correctly
  - no extra source-side rescue commands are needed after CDC setup
  - real destination tables converge correctly after restart
  - `runner verify` still passes against the real tables
- Lower-priority implementation concerns:
  - keep the long-lane tests readable
  - keep restart orchestration in typed support boundaries rather than shell glue inside the test body

## Interface And Boundary Decisions

- Keep all product CLIs unchanged.
- Introduce a dedicated runner lifecycle support boundary under `crates/runner/tests/support/`:
  - name can be `runner_process.rs` or similar
  - responsibilities:
    - spawn the `runner run` process
    - wait for health
    - kill the process intentionally
    - restart it against the same config and log paths
    - surface stdout and stderr on failure
- Keep `CdcE2eHarness` responsible for environment setup, config materialization, bootstrap execution, and durable SQL/query helpers.
- Add typed tracking-state snapshot helpers to `CdcE2eHarness` so restart tests do not embed raw helper-schema SQL inline.
- Introduce one dedicated destination-lock support boundary if needed:
  - name can be `destination_lock.rs` or equivalent
  - responsibility:
    - hold a real PostgreSQL lock long enough to block reconcile
    - release the lock deterministically
- Keep scenario-specific customer operations and assertions in `default_bootstrap_harness.rs`.
- Keep the long-lane test body focused on behavior:
  - bootstrap
  - mutate source
  - wait for durable pre-crash condition
  - crash
  - restart
  - assert convergence

## Public Contract To Establish

- One ignored long-lane test proves restart after helper persistence and before reconcile:
  - baseline migration bootstraps through the real bootstrap script and real runner
  - a live customer update lands in helper shadow state
  - tracking state shows the update was durably received but not yet reconciled
  - the runner is killed externally before the reconcile window
  - the runner is restarted with the same config
  - the real destination table converges to the correct updated row without any new source-side commands
  - `runner verify` succeeds
- One ignored long-lane test proves restart during reconcile:
  - baseline migration bootstraps through the same real path
  - a real PostgreSQL lock blocks reconcile from applying an update to the destination table
  - helper shadow state and received watermark advance while real tables remain stale
  - the runner is killed while reconcile is blocked
  - the lock is released and the runner is restarted
  - the real destination table converges correctly
  - resolved tracking does not regress and `runner verify` succeeds

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - `crates/runner/tests/support/e2e_harness.rs` currently mixes environment provisioning, runner lifecycle, and restart-observation responsibilities.
- Required cleanup during execution:
  - remove raw `Child` process orchestration from the generic harness body and give it a typed lifecycle owner
  - move any long-lived destination-lock logic into dedicated support rather than shelling it inline from a test
  - expose typed tracking-state helpers instead of repeating ad hoc helper-schema SQL in each scenario
- Smells this plan should explicitly avoid:
  - wrong-place-ism:
    - restart orchestration living in the customer scenario instead of generic support
  - mixed responsibilities:
    - one support file both building config and managing process/lock lifecycle state machines
  - remove-the-damn-helpers:
    - do not add tiny single-use helpers that only hide local test choreography

## Files And Structure To Add Or Change

- [ ] `crates/runner/tests/support/e2e_harness.rs`
  - replace ad hoc runner process ownership with typed lifecycle support
  - add restart helpers
  - add typed helper-schema tracking snapshots and polling helpers
- [ ] `crates/runner/tests/support/default_bootstrap_harness.rs`
  - add customer-scenario helpers for restartability assertions
  - keep helper-shadow and destination assertions readable
- [ ] `crates/runner/tests/default_bootstrap_long_lane.rs`
  - add the ignored restart-after-helper-persistence scenario
  - add the ignored restart-during-reconcile scenario
- [ ] `crates/runner/tests/support/runner_process.rs`
  - new support module for spawn, kill, restart, health wait, and log capture
- [ ] `crates/runner/tests/support/destination_lock.rs`
  - add only if needed for deterministic mid-reconcile blocking
- [ ] Product code changes are not expected
  - only real bug fixes are allowed if the RED slices expose one
  - likely hotspots only if the tests uncover a correctness bug:
    - `crates/runner/src/reconcile_runtime/mod.rs`
    - `crates/runner/src/tracking_state.rs`
    - `crates/runner/src/webhook_runtime/persistence.rs`

## TDD Execution Order

### Slice 1: Tracer Bullet For Restart After Helper Persistence

- [ ] RED: add one ignored failing long-lane test that bootstraps the default customers migration, uses a long reconcile interval, performs a live source update, waits until helper shadow state reflects the update while the real table is still stale, kills the runner, restarts it, and expects the destination to converge
- [ ] GREEN: add only the minimum runner lifecycle support and tracking-state observation needed for that scenario to pass through the real public path
- [ ] REFACTOR: keep lifecycle control in generic support and customer assertions in `default_bootstrap_harness.rs`

### Slice 2: Prove Durable Pre-Crash State Instead Of Timing Guesswork

- [ ] RED: strengthen the first test to assert a durable pre-crash checkpoint:
  - helper shadow row already updated
  - latest received resolved watermark has advanced
  - latest reconciled resolved watermark has not yet caught up
- [ ] GREEN: add the minimum typed tracking snapshot and polling helpers needed for those assertions
- [ ] REFACTOR: keep raw helper-schema SQL inside harness support, not in the test body

### Slice 3: Tracer Bullet For Crash During Reconcile

- [ ] RED: add one ignored failing long-lane test that bootstraps normally, acquires a real destination-side lock, performs a live source update, waits until helper state advances and reconcile is blocked, kills the runner mid-pass, releases the lock, restarts the runner, and expects destination convergence
- [ ] GREEN: add only the minimum lock support and restart orchestration needed to make that scenario pass
- [ ] REFACTOR: ensure the lock support owns the blocking session instead of embedding raw background process glue in the long-lane test

### Slice 4: Prove Restartability Of Real Tracking State

- [ ] RED: extend the reconcile-crash scenario to assert that tracking state does not regress across restart and eventually reaches the new resolved watermark after convergence
- [ ] GREEN: fix only the first real bug exposed by the tracking assertions
- [ ] REFACTOR: keep watermark comparison and snapshot formatting behind typed support helpers

### Slice 5: Real Verify And No Manual Rescue Regression

- [ ] RED: run `runner verify` after each restart scenario and assert the output reports a matched verdict for the real migrated table only
- [ ] GREEN: keep verification at the CLI boundary and avoid any alternate verification path
- [ ] REFACTOR: confirm the scenario uses no extra post-CDC source rescue commands beyond the planned live source mutation

### Slice 6: Full Repository Lanes And Final Boundary Review

- [ ] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [ ] GREEN: continue until every required lane passes cleanly
- [ ] REFACTOR: do one final `improve-code-boundaries` pass so runner lifecycle, destination blocking, generic harness setup, and customer-scenario assertions each own one clear responsibility

## TDD Guardrails For Execution

- Every new behavior assertion must fail before code changes are added. If an assertion already passes, replace it with the next uncovered behavior.
- Do not add hidden crash hooks, debug-only runner flags, or test-only runtime branches inside product code.
- Do not use extra source-side shell commands or SQL after CDC setup beyond the planned source data mutation that drives the restart scenario.
- Do not rely on blind sleeps to guess crash windows. Prefer helper-shadow state, tracking-state polling, lock ownership, and destination snapshots.
- Do not swallow startup, kill, restart, lock, or SQL errors. Any such failure is a real test failure.
- If a new support type has only one real caller and hides trivial local choreography, inline it instead of creating another fake boundary.

## Boundary Review Checklist

- [ ] Runner lifecycle control is no longer an ad hoc `Child` field buried inside the generic harness
- [ ] Durable tracking-state observation uses typed harness helpers rather than raw SQL copied into long-lane tests
- [ ] Mid-reconcile blocking lives in dedicated test support, not inline shell glue
- [ ] Customer-specific assertions stay in `default_bootstrap_harness.rs`
- [ ] Product code contains no restart-only test hooks
- [ ] No error path is swallowed or silently ignored

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long`
- [ ] One final `improve-code-boundaries` pass after all lanes are green
- [ ] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
