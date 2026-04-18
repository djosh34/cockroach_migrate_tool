# Plan: Build Drain-To-Zero And Cutover Readiness Checks

## References

- Task: `.ralph/tasks/story-09-verification-cutover/02-task-build-drain-to-zero-and-cutover-readiness-check.md`
- Previous task plan: `.ralph/tasks/story-09-verification-cutover/01-task-wrap-molt-verify-and-fail-on-log-detected-mismatches_plans/2026-04-19-molt-verify-wrapper-plan.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Current implementation:
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/molt_verify/mod.rs`
  - `crates/runner/src/tracking_state.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/error.rs`
- Current tests:
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
  - `crates/runner/tests/verify_contract.rs`
  - `crates/runner/tests/long_lane.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The public contract should stay mapping-scoped and operator-explicit, consistent with the existing `compare-schema`, `render-helper-plan`, and `verify` commands.
- “Drain to zero” means the destination has no unapplied progress left for the selected mapping:
  - `stream_state.latest_received_resolved_watermark` exists
  - `stream_state.latest_reconciled_resolved_watermark` exists
  - those two watermarks are equal
  - every selected table row in `_cockroach_migration_tool.table_sync_state` has `last_successful_sync_watermark` equal to that reconciled watermark
  - no selected table has a non-`NULL` `last_error`
- Readiness must be trustworthy for final handover. That means a stale historical verify artifact is not good enough on its own; the readiness command should invoke the existing MOLT verification boundary when the drain checks have already reached zero.
- This task should not add traffic switching or write-freeze enforcement. It should only expose the cutover readiness verdict and the reasons why it is or is not ready.
- If the first RED slice proves the operator contract should be artifact-based instead of live verify execution, or that the existing tracking tables cannot express drain-to-zero cleanly without schema changes, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Add one new CLI command:
  - `runner cutover-readiness --config <path> --mapping <id> --source-url <cockroach-url> [--allow-tls-mode-disable]`
- Keep this as a one-shot operator command, not part of `runner run`.
- The command will:
  - load the selected mapping and destination connection
  - query the destination tracking tables for one typed readiness snapshot
  - reduce that snapshot into a drain verdict with explicit reasons
  - only if the drain verdict is ready-to-verify, call the existing MOLT verify boundary
  - combine the drain verdict and verification verdict into one final cutover-readiness summary
- Keep `lib.rs` shallow:
  - parse CLI arguments
  - load config
  - dispatch to a dedicated cutover-readiness module
  - do not embed SQL, drain logic, or verify-orchestration decisions there
- Introduce one deep module under `runner` that owns readiness state:
  - destination query shape
  - typed snapshot rows
  - drain verdict reduction
  - verify orchestration once drain checks pass
  - final operator summary rendering

## Public Contract To Establish

- `runner cutover-readiness` reports one mapping-scoped verdict that combines:
  - watermark reconciliation status
  - helper-to-real-table drain status
  - MOLT verification status
- The command exits successfully and reports `ready=true` only when:
  - the selected mapping has received and reconciled a resolved watermark
  - every selected table has drained to that same watermark with no stored reconcile error
  - a fresh MOLT verify run reports equality
- The command reports `ready=false` and exits successfully for observable “not yet ready” states such as:
  - no received resolved watermark yet
  - received watermark ahead of reconciled watermark
  - one or more tables still lagging behind the reconciled watermark
  - one or more tables still carry a stored reconcile error
  - MOLT verify reports mismatches
- The command exits non-zero only for real command failures, such as:
  - unknown mapping
  - destination connection/query failure
  - missing tracking rows for the selected mapping
  - MOLT command spawn failure or hard process failure
- The operator-facing summary should include:
  - mapping id
  - latest received watermark
  - latest reconciled watermark
  - whether watermarks are aligned
  - whether table drain is complete
  - whether verification matched, mismatched, or was skipped because drain was incomplete
  - `ready=true|false`
  - concise failure reasons when not ready

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this task:
  - without a dedicated module, cutover-readiness would smear destination SQL queries, row decoding, drain verdict rules, and verify command orchestration across `lib.rs`, tests, and ad hoc helpers
- Required cleanup:
  - create one canonical `CutoverReadinessRequest -> CutoverReadinessSummary` boundary
  - keep helper-schema tracking queries and verdict reduction together so tests assert behavior through the public CLI instead of duplicating raw SQL semantics
  - keep the handoff from “drain checks passed” to “run MOLT verify” in one place, not scattered between CLI dispatch and tests
- Secondary cleanup:
  - avoid introducing parallel stringly verdict types such as `"aligned"`, `"drained"`, and `"matched"` in multiple modules; use one typed readiness model with one render path
  - reuse the existing verification boundary rather than reparsing MOLT JSON logs or reassembling command arguments from the readiness command
  - keep destination-connection usage on the existing `PostgresConnectionConfig` shape instead of inventing a second connection DTO for status reads

## Files And Structure To Add Or Change

- [x] `crates/runner/src/lib.rs`
  - add the `cutover-readiness` subcommand and dispatch to the dedicated readiness boundary
- [x] `crates/runner/src/error.rs`
  - add typed readiness errors for connection/query failures, missing tracking state, missing selected-table tracking rows, and invalid readiness state reads
- [x] `crates/runner/src/cutover_readiness/mod.rs`
  - new module that owns snapshot queries, drain verdict reduction, verify orchestration, and final summary rendering
- [x] `crates/runner/src/config/mod.rs`
  - add or reuse only the minimal accessors needed for selected mapping and verify config from the new command path
- [x] `crates/runner/tests/cli_contract.rs`
  - extend help coverage so the public CLI explicitly lists `cutover-readiness`
- [x] `crates/runner/tests/cutover_readiness_contract.rs`
  - new contract tests that run `runner cutover-readiness` end to end against a real temporary PostgreSQL instance plus a fake `molt` executable
- [x] `crates/runner/tests/long_lane.rs`
  - keep long-lane coverage green; no ignored test is planned initially

## TDD Execution Order

### Slice 1: Tracer Bullet For A Fully Ready Mapping

- [x] RED: add one failing `cutover_readiness_contract` test that bootstraps helper tracking state, seeds matching received/reconciled watermarks plus per-table sync watermarks, wires a clean fake `molt` script, and asserts `runner cutover-readiness` returns `ready=true`
- [x] GREEN: implement the minimum command dispatch, destination snapshot query, drain verdict reduction, and verify handoff needed for that success path
- [x] REFACTOR: extract the request/snapshot/result types into the dedicated readiness module so `lib.rs` stays orchestration-only

### Slice 2: Watermark Lag Reports Not Ready Without Running Verify

- [x] RED: add one failing contract test where `latest_received_resolved_watermark` is ahead of `latest_reconciled_resolved_watermark`, and assert `ready=false` with a reason that CDC/reconcile has not drained yet
- [x] GREEN: implement the false verdict for unresolved watermark lag
- [x] REFACTOR: keep watermark comparison and reason rendering in one reducer instead of duplicating string checks in tests

### Slice 3: Table-Level Drain Lag Reports Not Ready

- [x] RED: add one failing contract test where one selected table has a stale `last_successful_sync_watermark` or a persisted `last_error`, and assert `ready=false` with the lagging table called out
- [x] GREEN: implement per-table drain checks based on the selected mapping tables only
- [x] REFACTOR: keep table snapshot decoding and selected-table matching in one query/reducer path so missing or extra tracking rows do not leak into CLI code

### Slice 4: Verification Mismatch Blocks Readiness After Drain Reaches Zero

- [x] RED: add one failing contract test where the drain checks are ready but the fake `molt` verify run reports mismatch counters, and assert `ready=false` with verification mismatch context
- [x] GREEN: integrate the existing `molt_verify` boundary into the readiness command once drain checks pass
- [x] REFACTOR: avoid duplicate verify argument construction or duplicate mismatch-verdict parsing in the new module

### Slice 5: Missing Tracking State Fails Loudly

- [x] RED: add one failing contract test for a selected mapping that has no `stream_state` row or is missing one of its `table_sync_state` rows, and assert the command exits non-zero with a typed error
- [x] GREEN: implement loud tracking-state failures rather than silently treating missing rows as “not ready”
- [x] REFACTOR: keep missing-row detection in the readiness module so tests do not re-encode bootstrap invariants

### Slice 6: CLI Help And Required Lanes

- [x] RED: extend help coverage for `cutover-readiness`, then run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm readiness logic is not split across CLI, tracking-state helpers, and verify helpers

## TDD Guardrails For Execution

- Every readiness test slice must fail before code is added. If a proposed assertion already passes accidentally, replace it with the next uncovered behavior.
- Prefer integration-style contract tests through `runner cutover-readiness` over unit tests of private reducers.
- Do not swallow destination-query failures, missing tracking rows, persisted reconcile errors, or verification failures. If the chosen design makes a failure awkward to represent, add a typed error and keep it loud.
- Do not read a stale historical verify artifact to claim `ready=true`. The readiness verdict must include a fresh verify result from the current command execution.
- Do not broaden scope into traffic switching, API write freeze mechanics, or extra status persistence unless execution proves the current boundary insufficient.

## Boundary Review Checklist

- [x] No readiness SQL or verdict reduction lives in `lib.rs`
- [x] No `ready=true` verdict is possible without both drain completion and successful verification
- [x] No selected-table drain check can accidentally include helper-table names or omit a selected real table
- [x] No missing tracking row is downgraded into a fake “not ready yet” success
- [x] No duplicate verify command assembly or mismatch parsing is introduced outside the existing verification boundary
- [x] No second destination-connection DTO is introduced for readiness reads

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
