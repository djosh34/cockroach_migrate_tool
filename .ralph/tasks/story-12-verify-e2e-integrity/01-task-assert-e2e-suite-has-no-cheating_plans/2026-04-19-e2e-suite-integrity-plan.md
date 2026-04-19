# Plan: End-To-End Suite Integrity Without Cheating

## References

- Task: `.ralph/tasks/story-12-verify-e2e-integrity/01-task-assert-e2e-suite-has-no-cheating.md`
- Neighboring story-12 tasks that must stay separate:
  - `.ralph/tasks/story-12-verify-e2e-integrity/02-task-assert-single-container-tls-and-scoped-role-integrity.md`
  - `.ralph/tasks/story-12-verify-e2e-integrity/03-task-assert-no-post-setup-source-commands-in-e2e.md`
- Existing long-lane suite:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
  - `crates/runner/tests/long_lane.rs`
- Existing E2E support:
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/runner_process.rs`
  - `crates/runner/tests/support/webhook_chaos_gateway.rs`
  - `crates/runner/tests/support/destination_write_failure.rs`
- Runtime surfaces whose real path must remain the only path:
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/reconcile_runtime/upsert.rs`
  - `crates/runner/src/molt_verify.rs`
- Design and integrity requirements:
  - `designs/crdb-to-postgres-cdc/05_design_decisions.md`
  - `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown plus the design docs are treated as approval for the interface and behavior priorities in this turn.
- This task is about trust in the E2E suite itself. It should prove that the suite exercises the honest production path and should add guardrails that fail if the suite drifts into shortcuts.
- This task must not absorb the neighboring story-12 work:
  - task 02 owns single-container, TLS, real CockroachDB, and scoped PostgreSQL-role integrity
  - task 03 owns the stricter audit that no extra source-side commands occur after CDC setup completes
- The honest public path for this task remains:
  - `runner run --config <path>`
  - `source-bootstrap render-bootstrap-script --config <path>`
  - Cockroach changefeed webhook delivery over HTTPS
  - helper-shadow persistence first
  - reconcile into real destination tables second
  - `runner verify --config <path> --mapping <id> --source-url <url> [--allow-tls-mode-disable]`
- No new product-only or test-only bypass may be added:
  - no fake migration mode
  - no skip-webhook mode
  - no skip-reconcile mode
  - no alternate verify path
  - no direct destination-table seeding disguised as migration progress
- If the first RED slice proves that the current harness cannot surface trustworthy evidence for webhook, helper, reconcile, and verify without new ad hoc string parsing scattered through tests, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.

## Problem To Fix

- The current long-lane suite already covers many real-path behaviors, but its trust assertions are scattered:
  - webhook retry evidence lives in gateway-specific helpers
  - helper-first and reconcile-later progression is inferred piecemeal from waits and snapshots
  - verify assertions mostly inspect output text, while the MOLT wrapper itself is not audited
- That spread-out evidence is a code-boundary smell:
  - the suite has no single typed integrity boundary that says, "this migration progressed through webhook, helper shadow, reconcile, and real verify without a shortcut"
  - future tests could accidentally add convenience helpers or alternate paths and still pass existing assertions
- The `improve-code-boundaries` objective for this task is to flatten those scattered strings and waits into one support boundary that owns integrity evidence and cheat detection.

## Boundary And Interface Decisions

- Add one dedicated integrity-evidence support boundary under `crates/runner/tests/support/`.
  - suggested file: `e2e_integrity.rs`
  - responsibility:
    - collect typed evidence that a live mutation was observed on the real webhook path
    - prove helper shadow state advanced before real destination convergence
    - capture and parse MOLT-wrapper invocations so verify assertions use structured evidence instead of only output substrings
    - expose repository-level shortcut checks in one place
- Keep `CdcE2eHarness` responsible for low-level mechanics only:
  - Docker lifecycle
  - config and wrapper materialization
  - source and destination command execution
  - runner lifecycle
  - tracking-state polling
  - gateway observation plumbing
- Keep `DefaultBootstrapHarness` responsible for customer-specific scenarios and assertions.
- Flatten one current wrong boundary in `e2e_harness.rs`:
  - today `verify_migration` only returns output text
  - execution should add typed verify-audit data so E2E tests can assert the real MOLT command targeted the real selected tables and never helper tables
- Flatten one current wrong boundary in the long-lane suite:
  - integrity facts should not be re-derived separately in each scenario from arbitrary strings and sleeps
  - execution should move those facts behind a small typed integrity API, then let long-lane tests read like specifications
- Do not add any product runtime test hook. All integrity evidence must come from existing public behavior plus test support that observes commands and state honestly.

## Public Contract To Establish

- One fast repository contract test must fail if the E2E suite grows obvious shortcut paths.
  - suggested file: `crates/runner/tests/e2e_integrity_contract.rs`
  - focus:
    - only approved support modules may issue raw Docker/SQL/process commands for E2E orchestration
    - no E2E scenario file may run an alternate migration path or direct verify shortcut
    - no runtime or E2E support file may introduce an explicit fake/skip/bypass migration toggle for the public path
- One ignored long-lane test must prove the real live-update path explicitly.
  - bootstrap the default migration
  - perform one live source update
  - observe at least one real webhook delivery for that update
  - prove helper-shadow state changes before real destination convergence
  - prove the destination table converges only after reconcile catches up
  - run `runner verify`
  - assert structured verify evidence says the real selected table was verified and helper tables were not
- Existing recovery and churn long-lane tests should be refit to use the new typed integrity helpers where that reduces duplicate path-proof logic.

## TDD Approval And Behavior Priorities

- Highest-priority behaviors to prove:
  - the E2E suite cannot silently bypass webhook and reconcile while still claiming end-to-end coverage
  - a real live source mutation is observed on the webhook path, then persisted into helper shadow state, then reconciled into the real target table
  - `runner verify` in the E2E harness runs through the real MOLT wrapper against the real selected destination table set only
  - obvious fake/skip/bypass shortcut surfaces in E2E code are rejected by repository tests
- Lower-priority implementation concerns:
  - keep fast integrity checks in the default test lane
  - keep long-lane assertions readable by using typed evidence instead of free-form log parsing

## Vertical TDD Slices

### Slice 1: Tracer Bullet For Verify Audit Evidence

- [x] RED: add one failing fast contract test that requires structured verify evidence from the E2E harness instead of raw output-only checks
- [x] GREEN: extend the MOLT wrapper and harness support to log and parse real verify invocations with enough detail to assert target database and selected tables
- [x] REFACTOR: keep verify command parsing and helper-table exclusion logic in one typed support module instead of repeated string checks in long-lane tests

### Slice 2: Tracer Bullet For Real Live-Path Evidence

- [x] RED: add one ignored failing long-lane test that performs a live customer update and requires evidence for this exact sequence:
  - real webhook delivery observed
  - helper shadow customers reflect the update
  - real destination customers are still on the old value at that point
  - later the real destination customers converge
- [x] GREEN: add only the minimum typed integrity helper support needed to observe this honest progression without introducing sleeps or shortcuts
- [x] REFACTOR: move helper-first versus destination-later assertions behind a single support API instead of open-coded waits

### Slice 3: Fast Repository Shortcut Guard

- [x] RED: add a failing fast contract test that scans the E2E suite and runtime/support surfaces for disallowed shortcut markers or direct alternate orchestration paths
- [x] GREEN: encode the first minimal repository guardrails that reject obvious fake/skip/bypass migration surfaces while allowing the existing legitimate chaos/failure support
- [x] REFACTOR: keep allowlists and banned markers centralized in the integrity-contract support so the rule is easy to evolve without scattering string lists

### Slice 4: Strengthen Long-Lane Verify Assertions

- [x] RED: strengthen the new or existing default-bootstrap long-lane scenario so `runner verify` must satisfy typed integrity assertions:
  - MOLT ran through the harness wrapper
  - the target database matches the real destination database
  - only the selected real table set is verified
  - helper-shadow schema/table names are absent from the verify target evidence
- [x] GREEN: fix only the first real mismatch exposed by the new verify audit
- [x] REFACTOR: replace repeated `verify_output` substring checks in long-lane tests with the typed verify evidence helper wherever practical

### Slice 5: Improve-Code-Boundaries Pass

- [ ] RED: if integrity evidence is still duplicated across `default_bootstrap_long_lane.rs`, `default_bootstrap_harness.rs`, and `e2e_harness.rs`, add the next failing assertion that exposes the duplication
- [x] GREEN: consolidate the duplication into the typed integrity boundary
- [x] REFACTOR: remove leftover stringly path-proof helpers that no longer need to exist once the new boundary owns the data

### Slice 6: Full Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so integrity evidence, wrapper auditing, repository shortcut checks, gateway observation, and customer-scenario assertions each have one clear owner

## Guardrails For Execution

- Every new assertion must fail before the supporting code is added.
- Do not satisfy this task by only checking strings in already-passing output. At least one new assertion must use newly surfaced typed evidence.
- Do not use a fake MOLT binary in the E2E path. Fast contract tests may inspect wrapper text or logs, but the long-lane scenario must continue to use the real wrapper path.
- Do not add a product runtime branch, env var, CLI flag, or test-only code path to make E2E integrity easier to assert.
- Do not collapse this task into task 02 or task 03. If execution starts needing TLS-role proof or post-setup source-command auditing to make progress, stop and hand the work back to the correct task.
- Do not swallow failures from wrapper logging, gateway observation, tracking-state reads, Docker commands, or verify parsing.

## Boundary Review Checklist

- [x] Integrity evidence is owned by one typed support boundary instead of scattered log parsing
- [x] Long-lane tests read as behavior specifications, not orchestration scripts
- [x] Verify assertions use structured MOLT audit data, not only output substrings
- [x] Fast repository contract checks reject obvious fake/skip/bypass migration surfaces
- [x] Product code contains no E2E-only shortcut hook
- [x] No error path is swallowed or silently ignored

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
