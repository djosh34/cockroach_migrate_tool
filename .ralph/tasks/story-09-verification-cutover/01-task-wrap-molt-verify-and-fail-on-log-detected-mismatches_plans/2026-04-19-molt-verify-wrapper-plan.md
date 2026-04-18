# Plan: Wrap MOLT Verify And Fail On Log-Detected Mismatches

## References

- Task: `.ralph/tasks/story-09-verification-cutover/01-task-wrap-molt-verify-and-fail-on-log-detected-mismatches.md`
- Previous task plan: `.ralph/tasks/story-08-multi-db-orchestration/01-task-run-multiple-db-mappings-from-one-destination-container_plans/2026-04-18-multi-db-orchestration-plan.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/05_design_decisions.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Investigation: `designs/crdb-to-postgres-cdc/01_investigation_log.md`
- Investigation artifact: `investigations/cockroach-webhook-cdc/README.md`
- Investigation harness: `investigations/cockroach-webhook-cdc/scripts/run-molt-verify.sh`
- Current implementation:
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/config/mod.rs`
  - `crates/runner/src/config/parser.rs`
  - `crates/runner/src/error.rs`
- Current tests:
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/long_lane.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The public contract should stay operator-explicit and mapping-scoped, consistent with `compare-schema` and `render-helper-plan`:
  - `runner verify --config <path> --mapping <id> --source-url <cockroach-url>`
- This task does not need to fold Cockroach source credentials into `runner.yml`. The current runner config has no source connection section, and forcing a config-schema rewrite here would widen scope beyond the verification boundary.
- The selected mapping already defines the real migrated tables. The wrapper must pass those real table names to MOLT and must never derive `_cockroach_migration_tool` helper-table names.
- The existing `verify.molt.report_dir` config remains the artifact root for raw logs and machine-readable summaries.
- The destination connection contract is currently host/port/database/user/password only. The wrapper should initially mirror that contract by rendering the MOLT target URL from those fields rather than inventing a second destination-connection schema.
- Local and CI verification may need `--allow-tls-mode-disable`, as shown in the investigation harness. The safest public contract is an explicit CLI passthrough flag rather than hidden heuristics.
- If the first RED slice proves that MOLT cannot be driven correctly from `--source-url` plus the existing destination config, or that the public command must verify every mapping in one invocation rather than one mapping at a time, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Add one new CLI command:
  - `runner verify --config <path> --mapping <id> --source-url <cockroach-url> [--allow-tls-mode-disable]`
- Keep verification as a one-shot command, not part of `runner run`. Cutover orchestration is explicitly out of scope for this task.
- Introduce one deep verification module under `runner` that owns the entire wrapper:
  - build the typed MOLT invocation request
  - spawn the configured external command
  - capture combined output without swallowing process failures
  - parse JSON log lines into typed records
  - compute a typed verdict from summary counters and completion records
  - write raw log and summary artifacts under `report_dir`
  - render one operator-facing summary for stdout/stderr
- Keep `lib.rs` shallow:
  - parse CLI arguments
  - load config
  - dispatch to the verification module
  - do not assemble command arguments, parse JSON, or format summary details there
- Keep parsed MOLT log ownership inside the verification module:
  - no `serde_json::Value` escapes into CLI orchestration
  - no duplicate “raw record” and “summary row” shapes drift across tests and production code
  - the rest of the crate should see one typed verification result only

## Public Contract To Establish

- `runner verify` runs MOLT for exactly one selected mapping and exits non-zero when:
  - the MOLT process itself fails
  - no usable completion/summary evidence is produced
  - any per-table mismatch counters are non-zero
- A MOLT process exit code of `0` is not treated as success by itself.
- The wrapper must fail when any selected table reports non-zero values in counters that indicate data drift:
  - `num_missing`
  - `num_mismatch`
  - `num_extraneous`
  - `num_column_mismatch`
- The wrapper must use the selected mapping’s real tables only:
  - schema filter derived from `schema.table`
  - table filter derived from the mapping’s table list
  - no helper-schema tables included
- The command writes operator artifacts under `verify.molt.report_dir`, including:
  - raw captured log for the invocation
  - machine-readable summary JSON
  - enough metadata to inspect the mapping id, process exit code, and computed verification verdict
- The command prints a concise operator-facing summary that includes:
  - mapping id
  - selected tables
  - process exit code
  - computed verdict
  - per-table mismatch counts
  - artifact path

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this task:
  - without a dedicated module, CLI parsing, external process spawning, raw JSON parsing, verdict rules, and report rendering would get smeared across `lib.rs`, config accessors, and tests
- Required cleanup:
  - create one canonical `MoltVerifyRequest -> MoltVerifyResult` boundary
  - keep command-line construction in one place so tests assert behavior through the public `runner verify` interface instead of reimplementing argument assembly
  - keep mismatch verdict rules in one typed reducer instead of scattering `num_mismatch` string lookups across CLI code and tests
- Secondary cleanup:
  - keep report-path naming and artifact writing in the verification module, not in `main.rs` or test helpers
  - keep table-filter derivation from mapping config in one place so helper-table names cannot leak into verification
  - avoid introducing a second config DTO just for verification when the mapping config plus explicit `--source-url` already define the public inputs

## Files And Structure To Add Or Change

- [ ] `crates/runner/src/lib.rs`
- [x] `crates/runner/src/lib.rs`
  - add the `verify` subcommand and dispatch to the dedicated verification boundary
- [x] `crates/runner/src/config/mod.rs`
  - add the minimal accessors needed to read `verify.molt` and a selected mapping cleanly from the new command path
- [x] `crates/runner/src/error.rs`
  - add typed verification errors for command spawn, command failure, artifact write, malformed log records, missing summary/completion evidence, and mismatch verdicts
- [x] `crates/runner/src/molt_verify/mod.rs`
  - new module that owns request building, process execution, JSON-line parsing, verdict aggregation, artifact writing, and final summary formatting
- [x] `crates/runner/tests/cli_contract.rs`
  - extend help coverage so the public CLI explicitly lists `verify`
- [x] `crates/runner/tests/verify_contract.rs`
  - new contract tests that execute `runner verify` end-to-end against a fake `molt` executable emitting real investigation-shaped JSON logs
- [x] `crates/runner/tests/config_contract.rs`
  - keep config validation/output coverage green if the new command needs small label or accessor adjustments
- [x] `crates/runner/tests/long_lane.rs`
  - no new ignored long test is planned initially; `make test-long` must still pass unchanged unless execution reveals a real need

## TDD Execution Order

### Slice 1: Tracer Bullet For A Clean Verification Pass

- [x] RED: add one failing `verify_contract` test that runs `runner verify` with a fake `molt` executable that exits `0` and emits investigation-shaped summary/completion JSON for a clean match
- [x] GREEN: implement the minimum command dispatch, external process execution, JSON-line parsing, and success summary needed for that mapping-scoped command to pass
- [x] REFACTOR: extract the command request/result types into the dedicated verification module so `lib.rs` stays orchestration-only

### Slice 2: Exit Code `0` Still Fails On Mismatch Counters

- [x] RED: add one failing contract test where the fake `molt` process exits `0` but emits a per-table summary with `num_mismatch = 1`, and assert `runner verify` exits non-zero with a mismatch summary
- [x] GREEN: implement verdict reduction from summary counters rather than raw process status
- [x] REFACTOR: centralize counter extraction and verdict rules so tests do not duplicate string-key lookups

### Slice 3: Real Tables Only, Not Helper Tables

- [x] RED: add one failing contract test that captures the spawned fake `molt` arguments and asserts the wrapper passes only the selected mapping’s real schema/table filters, with no `_cockroach_migration_tool` references
- [x] GREEN: derive schema and table filters from `mappings.source.tables`
- [x] REFACTOR: keep filter construction in one request-builder path so helper-table naming cannot leak into verification

### Slice 4: Artifact Writing And Operator Summary

- [x] RED: add failing coverage that expects `verify.molt.report_dir` to receive a raw log file plus summary JSON for the invocation, and expects stdout/stderr to include the artifact location and computed verdict
- [x] GREEN: write the raw output and summary artifacts and render the concise command summary
- [x] REFACTOR: keep artifact naming and summary formatting inside the verification module rather than inline in command dispatch

### Slice 5: Loud Failure For Malformed Or Incomplete MOLT Output

- [x] RED: add failing coverage for a fake `molt` run that exits `0` but omits usable summary/completion evidence or emits malformed JSON-only noise, and assert the wrapper fails loudly instead of silently treating the run as success
- [x] GREEN: implement typed parse/integrity failures for incomplete verification evidence
- [x] REFACTOR: keep raw-record parsing tolerant of non-JSON lines but strict about the minimum evidence required for a trustworthy verdict

### Slice 6: Optional TLS-Disable Passthrough

- [x] RED: add failing coverage that uses the explicit CLI passthrough flag and asserts the fake `molt` process receives `--allow-tls-mode-disable`
- [x] GREEN: thread the explicit passthrough flag into the MOLT command builder
- [x] REFACTOR: keep all MOLT argument ordering and escaping inside the request builder so tests only verify the public behavior

### Slice 7: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm verification logic is not split across CLI, config, and test helpers

## TDD Guardrails For Execution

- Every verification test slice must fail before code is added. If a proposed success-path assertion already passes accidentally, replace it with the next uncovered behavior.
- Prefer integration-style contract tests through `runner verify` over unit tests of private parser helpers.
- Do not swallow malformed log lines, missing summary evidence, artifact-write failures, or process-spawn failures. If the chosen design makes a failure awkward to represent, add a typed error and keep it loud.
- Do not add a second runner config schema just to support verification. Use the existing mapping config plus explicit CLI inputs unless execution proves that boundary wrong.

## Boundary Review Checklist

- [x] No verification-specific command assembly lives in `lib.rs`
- [x] No MOLT success verdict relies on process exit code alone
- [x] No helper-schema table names can leak into the verify table filter
- [x] No raw `serde_json::Value` parsing contract escapes the verification module
- [x] No artifact-write or parse failure is downgraded into fake success
- [x] No duplicate mismatch-counter logic exists across production code and tests

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
