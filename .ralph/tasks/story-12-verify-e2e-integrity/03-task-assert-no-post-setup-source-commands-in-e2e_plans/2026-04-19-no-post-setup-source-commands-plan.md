# Plan: End-To-End Integrity Without Post-Setup Source Intervention

## References

- Task: `.ralph/tasks/story-12-verify-e2e-integrity/03-task-assert-no-post-setup-source-commands-in-e2e.md`
- Previous story-12 plans:
  - `.ralph/tasks/story-12-verify-e2e-integrity/01-task-assert-e2e-suite-has-no-cheating_plans/2026-04-19-e2e-suite-integrity-plan.md`
  - `.ralph/tasks/story-12-verify-e2e-integrity/02-task-assert-single-container-tls-and-scoped-role-integrity_plans/2026-04-19-single-container-tls-scoped-role-integrity-plan.md`
- Existing E2E suites and support:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
  - `crates/runner/tests/e2e_integrity_contract.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/composite_pk_exclusion_harness.rs`
  - `crates/runner/tests/support/multi_mapping_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
- Design requirements:
  - `designs/crdb-to-postgres-cdc/05_design_decisions.md`
  - `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown plus the design docs are treated as approval for the interface and behavior priorities in this turn.
- This task is about post-setup source-side integrity specifically:
  - bootstrap commands are still explicit and audited
  - live source mutations are allowed when they model customer activity
  - post-setup source commands must not be needed to make migration progress happen
- This task must stay separate from neighboring story-12 work:
  - task 01 owns generic anti-cheating and typed verify/runtime evidence
  - task 02 owns single-container, TLS, Cockroach, and scoped-role runtime shape
- If the first RED slice proves the current harness cannot distinguish legitimate customer writes from forbidden migration-driving source intervention without inventing a much richer workload DSL than the current tests justify, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Problem To Fix

- The current suite audits bootstrap source commands through the Cockroach wrapper log, but post-bootstrap source activity escapes that boundary:
  - `bootstrap_migration()` uses the wrapper-backed shell script
  - later source SQL goes through `CdcE2eHarness::execute_source_sql()`
  - `execute_source_sql()` calls `DockerEnvironment::exec_cockroach_sql()` directly
- That is a code-boundary smell:
  - there is no single typed audit that owns all source-side commands across setup and post-setup phases
  - scenario code can keep relying on raw source SQL after CDC setup and the suite will not prove whether those commands are legitimate workload mutations or migration-driving intervention
  - the suite cannot currently fail on hidden post-setup source reads, helper SQL, or admin commands because it does not record them
- The `improve-code-boundaries` target for this task is to flatten source-command issuance and source-command auditing behind one typed support boundary instead of split logging and raw direct execution.

## Boundary And Interface Decisions

- Extend `crates/runner/tests/support/e2e_integrity.rs` with one typed source-command audit boundary instead of adding more string parsing to scenario files.
  - suggested public surface:
    - `SourceCommandAudit`
    - `SourceCommandPhase` with at least `Bootstrap` and `PostSetup`
    - `PostSetupSourceAudit`
- Flatten the wrong boundary in `crates/runner/tests/support/e2e_harness.rs`:
  - all Cockroach source command execution should pass through one audited path
  - bootstrap commands and post-setup commands should be recorded in the same log or typed record stream
  - customer-facing harnesses should expose named workload helpers, not a public generic post-setup escape hatch
- Keep low-level Docker execution in `DockerEnvironment`, but do not let scenario-facing code bypass the audit when issuing source commands.
- The typed post-setup audit should be able to prove:
  - which source commands happened after CDC setup completed
  - whether they were workload DML against application tables
  - that no post-setup command reissued bootstrap/admin behavior such as:
    - `SET CLUSTER SETTING`
    - `SELECT cluster_logical_timestamp()`
    - `CREATE CHANGEFEED`
    - helper-schema or system-schema side-channel SQL
    - ad hoc shell commands against the source outside the audited Cockroach path

## Public Contract To Establish

- One fast repository integrity contract test should fail if the E2E suite regresses to unaudited post-setup source command execution.
  - scenario files and customer-facing harnesses must not own raw Cockroach source command plumbing
  - the typed integrity boundary must expose post-setup source-command audit support
- One ignored long-lane scenario should explicitly prove that migration progress after CDC setup depends on the destination runtime, not further source-side intervention.
  - bootstrap the migration
  - capture a post-setup baseline from the typed source-command audit
  - issue one legitimate live source mutation through a named workload helper
  - wait for helper-shadow and real destination convergence
  - assert the post-setup audit contains only the expected workload mutation commands
  - assert the post-setup audit contains no extra source admin/setup/helper commands while the destination catches up
- Existing multi-table and multi-mapping long-lane helpers should be refit to use the same typed source-command audit where that removes duplicated bootstrap-vs-live command reasoning.

## TDD Approval And Behavior Priorities

- Highest-priority behaviors to prove:
  - the E2E suite records and audits all source-side commands after CDC setup
  - migration progress after setup does not depend on extra source admin/setup/helper commands
  - legitimate live customer writes remain possible and are distinguished from forbidden intervention
  - repository contract checks reject new unaudited or scattered post-setup source command paths
- Lower-priority implementation concerns:
  - keep scenario code readable by asserting typed source-command evidence rather than raw log substrings
  - remove or narrow generic `execute_source_sql` exposure where it muddies the boundary

## Vertical TDD Slices

### Slice 1: Tracer Bullet For Typed Post-Setup Source Audit

- [x] RED: add one failing fast integrity-contract test that requires a typed post-setup source audit boundary in `e2e_integrity.rs` and forbids scenario-facing reliance on scattered raw source-command checks
- [x] GREEN: add the minimum typed source-command audit support and wire all source command execution through the audited path
- [x] REFACTOR: keep phase parsing, statement capture, and forbidden-marker rules inside the integrity boundary instead of repeating them in harnesses or tests

### Slice 2: Honest Default Long-Lane Proof

- [x] RED: add or strengthen one ignored default-bootstrap long-lane test so it fails unless the suite can prove that, after CDC setup, destination convergence happens without extra source setup/admin/helper commands
- [x] GREEN: expose only the minimum audit hooks needed to assert the allowed post-setup workload commands and reject forbidden intervention
- [x] REFACTOR: move post-setup source-command assertions behind a small typed API so the scenario reads like behavior, not log parsing

### Slice 3: Narrow The Source Command Escape Hatch

- [x] RED: add a fast contract test that fails while customer-facing harnesses still expose a broad raw post-setup source SQL escape hatch
- [x] GREEN: narrow or hide the generic source command helper behind named workload methods and the audited support boundary
- [x] REFACTOR: remove leftover duplicate helper methods or log readers that no longer need to exist once one owner records source commands

### Slice 4: Multi-Scenario Coverage

- [x] RED: add the next failing assertion in composite-key or multi-mapping support that requires the same typed post-setup audit for legitimate live writes
- [x] GREEN: fit those support modules to the shared audit boundary without introducing a second source-command logger
- [x] REFACTOR: keep allowlists or expected post-setup statement groups centralized so scenario helpers do not each invent their own command parsing

### Slice 5: Forbidden Post-Setup Source Behavior Rules

- [x] RED: add failing assertions for the first forbidden post-setup behaviors that are currently unobservable:
  - re-running bootstrap/admin SQL after setup
  - source-side helper or tracking SQL meant to make migration pass
  - source-side shell execution outside the audited Cockroach SQL path
- [x] GREEN: make the audit fail loudly on those behaviors with clear diagnostics
- [x] REFACTOR: keep the forbidden behavior classification typed and centralized

### Slice 6: Improve-Code-Boundaries Pass

- [x] RED: if source-command ownership is still split awkwardly across `e2e_harness.rs`, `default_bootstrap_harness.rs`, `composite_pk_exclusion_harness.rs`, and `multi_mapping_harness.rs`, add the next failing assertion that exposes the split
- [x] GREEN: consolidate source-command ownership behind the audited boundary and named workload helpers
- [x] REFACTOR: remove leftover stringly bootstrap-vs-live command checks that no longer need to exist

### Slice 7: Full Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so source-command execution, source-command auditing, and scenario workload helpers each have one clear owner

## Guardrails For Execution

- Every new assertion must fail before the supporting code is added.
- Do not satisfy this task by banning all post-setup source writes. Customer workload mutations after setup remain legitimate test behavior.
- Do not satisfy this task with repository grep checks alone. At least one ignored long-lane scenario must assert typed post-setup source-command evidence from the running harness.
- Do not add a product runtime flag, hidden helper command, or silent audit bypass to make the suite easier to prove.
- Do not swallow source-command logging, source SQL execution, or audit parsing failures.
- If execution discovers that post-setup progress still relies on source-side bootstrap/admin/helper commands, treat that as a design failure and switch this plan back to `TO BE VERIFIED`.

## Boundary Review Checklist

- [x] One typed support boundary owns source-command evidence across bootstrap and post-setup phases
- [x] Scenario files read as behavior specifications, not raw source-command scripts
- [x] Legitimate live source writes remain possible without opening a generic unaudited escape hatch
- [x] Post-setup bootstrap/admin/helper commands are rejected explicitly
- [x] No source-command failure path is swallowed or silently ignored

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
