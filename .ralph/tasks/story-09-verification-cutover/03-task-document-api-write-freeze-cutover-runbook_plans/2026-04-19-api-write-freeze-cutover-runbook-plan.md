# Plan: Document API Write-Freeze Cutover Runbook

## References

- Task: `.ralph/tasks/story-09-verification-cutover/03-task-document-api-write-freeze-cutover-runbook.md`
- Previous task plan: `.ralph/tasks/story-09-verification-cutover/02-task-build-drain-to-zero-and-cutover-readiness-check_plans/2026-04-19-drain-to-zero-cutover-readiness-plan.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Investigation: `designs/crdb-to-postgres-cdc/01_investigation_log.md`
- Current operator docs:
  - `README.md`
- Current tests:
  - `crates/runner/tests/cli_contract.rs`
  - `crates/runner/tests/config_contract.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The selected cutover model is already fixed by design and previous story work:
  - keep PostgreSQL continuously shadowing CockroachDB
  - run repeated parity checks during that shadowing period
  - block writes at the API boundary when handover starts
  - wait for drain-to-zero
  - require a final clean verify result
  - switch traffic only after those checks pass
- This task is documentation-first. It should not add new CLI commands or new runtime state.
- The public operator boundary for this task is the existing root `README.md`, not design notes or source-code spelunking.
- The runbook should rely on the public `runner verify` and `runner cutover-readiness` commands instead of telling the operator to inspect helper-schema internals such as `_cockroach_migration_tool.stream_state` or `_cockroach_migration_tool.table_sync_state`.
- If the first RED slice proves that the runbook cannot stay concise and actionable inside the root `README.md` without becoming muddy, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Add one new root-README section dedicated to the cutover path:
  - `## Write-Freeze Cutover Runbook`
- Keep the runbook operator-facing and mapping-scoped:
  - repeat `runner verify --config <path> --mapping <id> --source-url <cockroach-url>` while shadowing
  - block writes at the API layer
  - run `runner cutover-readiness --config <path> --mapping <id> --source-url <cockroach-url>` until it reports `ready=true`
  - run one final `runner verify`
  - switch traffic only after verification still reports equality
- Keep `README.md` as the single canonical operator sequence for cutover:
  - no second competing runbook in generated artifacts
  - no contradictory order spread across top-level README paragraphs
- Add one integration-style documentation contract test that reads the real repository `README.md` and asserts the required runbook content and order through the public text the operator actually sees.

## Public Contract To Establish

- The README explains that parity checks are not one final surprise step; they are repeated during the continuous-shadowing period before handover.
- The README explains that writes are blocked at the API boundary before waiting for drain-to-zero.
- The README explains that `runner cutover-readiness` is the public readiness signal, so the operator does not need to query internal tracking tables directly.
- The README explains that one final `runner verify` still runs after drain-to-zero before switching traffic.
- The README explains the switch criteria directly:
  - writes are frozen
  - readiness has drained to zero
  - final verify reports equality
  - only then is traffic switched to PostgreSQL
- The README stays concise and action-oriented:
  - minimal prose
  - no generic migration theory
  - no “inspect the code” or “look up the helper tables” requirement

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this task:
  - the operator cutover story is currently smeared across design documents and CLI behavior, while the public README does not yet own the handover workflow as one direct runbook
- Required cleanup:
  - move the operator-facing cutover sequence into one dedicated README section
  - keep internal drain/watermark mechanics behind the `cutover-readiness` command instead of leaking database-table inspection into the docs
  - remove any stale or indirect wording in the README that makes the user infer the order of write freeze, drain, verify, and switch
- Secondary cleanup:
  - keep the doc assertions in one README contract test instead of scattering README expectations into unrelated config-artifact tests
  - do not introduce a separate docs-only abstraction layer or duplicate runbook text in multiple files unless execution proves the root README boundary is wrong

## Files And Structure To Add Or Change

- [x] `README.md`
  - add the write-freeze cutover runbook section with the exact operator sequence and public commands
- [x] `crates/runner/tests/readme_contract.rs`
  - new contract test file that reads the repository README and asserts the cutover runbook content and ordering

## TDD Execution Order

### Slice 1: Tracer Bullet For A Visible Cutover Runbook

- [x] RED: add one failing `readme_contract` test that asserts the root `README.md` contains a dedicated write-freeze cutover runbook section and mentions repeated `runner verify` checks during the shadowing period
- [x] GREEN: add the new README section with a concise shadowing-period parity-check step
- [x] REFACTOR: keep README loading and common assertion helpers local to the new documentation contract test so other test files do not absorb root-README concerns

### Slice 2: Exact Handover Order And Public Commands

- [x] RED: extend the README contract test so it fails unless the documented order is:
  - API write freeze
  - drain-to-zero readiness wait through `runner cutover-readiness`
  - final `runner verify`
  - traffic switch criteria
- [x] GREEN: update the README so the runbook states that order explicitly and uses only public commands, not helper-schema inspection
- [x] REFACTOR: tighten the prose so the section stays directly actionable and does not duplicate generic theory already captured in design documents

### Slice 3: Explicit Switch Criteria And Failure Rule

- [x] RED: extend the README contract test so it fails unless the runbook explicitly says not to switch traffic until readiness reports drained and final verify reports equality
- [x] GREEN: add the final gate wording to the runbook and make the switch criteria unmistakable
- [x] REFACTOR: remove any stale README wording elsewhere that could suggest a looser or differently ordered cutover flow

### Slice 4: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm the operator cutover contract is owned by the README and not split across unrelated docs/tests

## TDD Guardrails For Execution

- Every documentation slice must start RED. If a proposed assertion already passes, replace it with the next missing observable behavior in the runbook.
- Prefer one root-README contract test over unit-style tests of string helpers.
- Do not weaken the docs into vague statements like “cut over when ready.” The runbook must spell out the exact operator-visible order.
- Do not tell the operator to inspect source code, helper schemas, or design docs to complete the cutover path.
- Do not add generic migration theory. This task is about the concrete cutover runbook only.

## Boundary Review Checklist

- [x] The root `README.md` contains one canonical write-freeze cutover sequence
- [x] The runbook uses public CLI commands instead of leaking helper-schema internals
- [x] The README documents repeated parity checks before handover, not only one final verify
- [x] The README documents write freeze before drain, and final verify before switch
- [x] No stale or contradictory cutover wording remains in the touched README sections
- [x] README expectations are isolated in a dedicated documentation contract test

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
