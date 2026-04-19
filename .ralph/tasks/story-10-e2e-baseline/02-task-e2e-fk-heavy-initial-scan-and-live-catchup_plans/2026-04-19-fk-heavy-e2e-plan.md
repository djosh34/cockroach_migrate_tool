# Plan: End-To-End FK-Heavy Initial Scan And Live Catch-Up

## References

- Task: `.ralph/tasks/story-10-e2e-baseline/02-task-e2e-fk-heavy-initial-scan-and-live-catchup.md`
- Previous story-10 plan: `.ralph/tasks/story-10-e2e-baseline/01-task-e2e-default-database-bootstrap-from-scratch_plans/2026-04-19-default-bootstrap-e2e-plan.md`
- Reconcile coverage: `crates/runner/tests/reconcile_contract.rs`
- Existing long-lane baseline: `crates/runner/tests/default_bootstrap_long_lane.rs`
- Existing long-lane support: `crates/runner/tests/support/e2e_harness.rs`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Investigation: `investigations/cockroach-webhook-cdc/README.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task is a new real ignored long-lane scenario, not a unit-test expansion. It must exercise the actual public operator flow:
  - `runner run --config <path>`
  - `source-bootstrap render-bootstrap-script --config <path>`
  - `runner verify --config <path> --mapping <id> --source-url <url> [--allow-tls-mode-disable]`
- The destination tables must remain real constrained tables for the whole run:
  - primary keys enabled
  - foreign keys enabled
  - no test-only disable/enable cycle
- The source scenario should use a true FK graph with at least:
  - `public.parents`
  - `public.children` referencing `parents`
  - `public.grandchildren` referencing `children`
- The source must contain preexisting rows before CDC setup, and the test must also mutate the source after the changefeed is live so one scenario proves both initial scan and live catch-up.
- The current baseline harness is too scenario-specific. If this task simply keeps adding FK-specific helpers into `DefaultBootstrapHarness`, the long-lane support boundary will get muddier. The execution turn should fix that instead of papering over it.
- If the first RED slice proves the FK-heavy scenario needs a different public CLI or a different helper-shadow/reconcile artifact contract than the one already shipped, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- The task markdown, acceptance criteria, and existing story-10 conventions are treated as approval for the public interface and the required behaviors.
- Highest-priority behaviors to prove:
  - initial scan of a populated FK graph lands safely while real target constraints stay enabled
  - live inserts/updates/deletes after feed start also converge safely
  - repeated reconcile passes remain idempotent after convergence
  - MOLT verify checks only the real migrated tables and reports a match

## Interface And Boundary Decisions

- Keep all shipped CLI contracts unchanged.
- Keep the FK-heavy scenario in one new ignored long test rather than spreading one operator journey across multiple files.
- Introduce one more general long-lane support boundary so scenario files stay behavior-focused:
  - extract a reusable CDC E2E harness from the current baseline-only support
  - keep scenario-specific seed data and assertions outside the Docker/process/bootstrap plumbing
- Keep helper-shadow inspection and source-mutation helpers centralized in support code instead of duplicating raw SQL snippets in the test file.
- Keep verification assertions at the public command boundary. The test should observe `runner verify` output and real destination tables, not internal Rust functions.

## Public Contract To Establish

- One ignored long-lane FK-heavy scenario proves the selected helper-shadow plus reconcile design under real CDC:
  - source schema contains parent, child, and grandchild tables with real foreign keys
  - destination schema contains the same real constrained tables
  - the source is seeded before CDC setup
  - the rendered bootstrap script is used to start CDC
  - initial scan rows land in helper shadow tables first
  - reconcile applies them into the real destination tables in FK-safe order
  - the test performs additional live source mutations after CDC starts
  - reconcile catches up again without violating destination constraints
  - repeated reconcile passes do not create duplicates or regress state
  - `runner verify` succeeds and refers only to the real migrated tables

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - `crates/runner/tests/support/e2e_harness.rs` currently mixes generic Docker/process/config/bootstrap orchestration with baseline-scenario assumptions such as customers-only assertions and one-table config generation.
- Required cleanup during execution:
  - split generic long-lane orchestration from scenario-specific fixtures/assertions
  - make the support layer own:
    - Docker lifecycle
    - temp config generation
    - wrapper binaries
    - polling helpers
    - real DB query helpers
  - make each ignored test file own only:
    - scenario seed/mutation intent
    - observable assertions
- Avoid a second layer of stringly table-name plumbing if the existing helper-plan or selected-table metadata can be reused to drive helper-table queries and verify expectations.

## Files And Structure To Add Or Change

- [x] `crates/runner/tests/support/e2e_harness.rs`
  - extracted a reusable multi-table CDC harness from the old baseline-only support
- [x] `crates/runner/tests/support/default_bootstrap_harness.rs`
  - kept the default scenario on a thin wrapper boundary so baseline-only assertions do not leak back into shared support
- [x] `crates/runner/tests/default_bootstrap_long_lane.rs`
  - kept the baseline scenario working on the refactored shared support boundary and added the FK-heavy ignored end-to-end contract in the same integration crate
- [x] Separate `crates/runner/tests/fk_heavy_long_lane.rs` was not kept after execution
  - the FK-heavy scenario was intentionally collapsed into `default_bootstrap_long_lane.rs` because separate integration crates made the shared helper surface look dead under `clippy -D warnings`
- [x] `crates/runner/tests/long_lane.rs`
  - no functional wiring change was required; only formatting churn remained after the repo-wide `cargo fmt`
- [x] Product code did not need changes
  - likely hotspots if needed:
    - `crates/runner/src/reconcile.rs`
    - `crates/runner/src/helper_plan.rs`
    - `crates/runner/src/molt_verify/mod.rs`
- [x] No CLI surface expansion was needed
  - no new user-facing flag or test-only hook was introduced

## TDD Execution Order

### Slice 1: Tracer Bullet For FK-Heavy Initial Scan

- [x] RED: add one ignored failing long-lane test that seeds a parent/child/grandchild source schema before CDC setup, starts `runner run`, executes the rendered bootstrap script, and waits for the real destination tables to match the seeded FK graph
- [x] GREEN: implement only the minimum harness refactor/support needed for that one scenario to converge through the real webhook plus reconcile path
- [x] REFACTOR: keep the new harness generic enough that the existing default-bootstrap ignored test can share it without inheriting FK-specific assumptions

### Slice 2: Prove Real Constraints Stay Enabled

- [x] RED: extend the FK-heavy scenario to assert the destination real tables keep PK/FK constraints present throughout the run and that the final state exists in constrained real tables rather than only helper shadows
- [x] GREEN: no product fix was needed; the existing helper-shadow plus reconcile flow passed once the real scenario was expressed through the harness
- [x] REFACTOR: centralized destination constraint inspection in support code so the scenario stays readable

### Slice 3: Prove Live Catch-Up After Initial Scan

- [x] RED: after the changefeed is live, apply additional source mutations covering at least:
  - parent insert
  - child insert referencing that parent
  - grandchild insert referencing that child
  - one update on an existing row
  - one delete that forces reverse dependency cleanup
- [x] GREEN: the real pipeline converged again without bypassing the changefeed path
- [x] REFACTOR: kept source-mutation execution on the harness boundary so the test file does not accumulate ad hoc bootstrap plumbing

### Slice 4: Prove Repeated Reconcile Is Safe

- [x] RED: extend the scenario to observe stable destination state across multiple reconcile intervals after convergence
- [x] GREEN: no product fix was needed; repeated reconcile kept the converged snapshot stable
- [x] REFACTOR: reused shared destination query helpers instead of duplicating Docker/query wiring across tests

### Slice 5: Real MOLT Verify On Real Tables Only

- [x] RED: run `runner verify` for the FK-heavy mapping and assert the output reports a match for the migrated real tables only, not helper-shadow tables
- [x] GREEN: the existing verify path passed on the real scenario once the generic harness could drive the real destination contract
- [x] REFACTOR: kept verify-command execution on the shared harness so long-lane scenarios do not drift in wrapper setup

### Slice 6: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: every required lane passed cleanly
- [x] REFACTOR: one final `improve-code-boundaries` pass kept the shared harness generic and the baseline-only wrapper isolated

## TDD Guardrails For Execution

- Every proposed behavior assertion must fail before code changes are added. If a candidate assertion already passes, replace it with the next uncovered behavior.
- Do not fake the webhook path, the reconcile loop, or MOLT.
- Do not weaken destination constraints to make the scenario pass.
- Do not silently ignore Docker, source-bootstrap, runner, or verify failures.
- Do not add raw source-side commands after CDC setup other than the planned live data mutations that model normal application writes.
- Do not let baseline-only assumptions remain hidden in shared support if the new scenario proves those assumptions are the real coupling problem.

## Boundary Review Checklist

- [x] Shared long-lane support no longer assumes one table or one hard-coded customers scenario
- [x] Ignored long tests read as operator behavior, not Docker shell glue
- [x] Source live mutations are explicit and scenario-owned, not mixed into bootstrap orchestration
- [x] Helper-shadow inspection is centralized instead of duplicated string SQL in test files
- [x] MOLT verify assertions remain at the CLI/output boundary
- [x] No error from the real CDC path, reconcile loop, or Dockerized wrappers is swallowed

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
