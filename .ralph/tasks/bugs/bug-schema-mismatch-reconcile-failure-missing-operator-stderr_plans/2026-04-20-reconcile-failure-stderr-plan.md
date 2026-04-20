# Plan: Surface Schema-Mismatch Reconcile Failures Through Operator Stderr

## References

- Task:
  - `.ralph/tasks/bugs/bug-schema-mismatch-reconcile-failure-missing-operator-stderr.md`
- Current long-lane and audit coverage:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
- Current runner stderr capture support:
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/runner_process.rs`
- Current runtime and logging boundary:
  - `crates/runner/src/main.rs`
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/error.rs`
  - `crates/operator-log/src/lib.rs`
- Current failure persistence boundary:
  - `crates/runner/src/tracking_state.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The bug task markdown is sufficient approval for the public behavior direction and test priority in this planning turn.
- This planning artifact is now verified and should be used as the execution contract for the next red-green implementation turn.
- The failure is already real and partially covered by in-progress long-lane work:
  - helper state persists
  - `last_error` persists
  - reconcile remains bounded
  - runner stays alive
  - operator stderr is the missing public signal
- The fix must preserve bounded failure semantics.
  - Do not turn this into a process-fatal error just to get stderr output.
- The logging contract should remain one operator-facing stderr stream that supports both text and JSON modes honestly.
- If the first RED slice shows the current event-emission shape cannot express a live reconcile failure without mixing direct stderr writes into business logic, execution must switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - a schema-mismatch reconcile apply failure emits operator-visible stderr context while the runner keeps running
  - the emitted context includes mapping id, real table, reconcile phase, and database error detail
  - helper persistence and `last_error` remain intact
  - ingress delivery count stays bounded at one attempt for the schema-mismatch path
- Lower-priority concerns:
  - exact text formatting beyond stable key phrases
  - broad logging redesign outside the runner runtime path

## Current State Summary

- The current bug shape is not a missing persistence bug.
  - `tracking_state::persist_reconcile_failure` already records a table-specific `last_error`.
  - the schema-mismatch audit already reads that persisted state successfully.
- The operator-visible gap is a live runtime logging gap.
  - `runner::execute` exposes an `emit_event` callback and already uses it for `runtime.starting`.
  - `main.rs` only writes callback events to stderr when `--log-format json` is selected.
  - the default text-mode runtime therefore drops callback events entirely.
- The bounded reconcile failure path never becomes a top-level command failure.
  - `reconcile_runtime::run_reconcile_pass` rolls back, persists failure state, records metrics, and returns `ApplyFailedRecorded`.
  - because the runner continues serving, `main.rs` never emits a `command.failed` event for this case.
- The current long-lane work has already exposed the honest acceptance boundary:
  - `ignored_long_lane_recovers_after_reconcile_transaction_failure` now expects stderr context
  - `SchemaMismatchAudit::assert_bounded_operator_action` expects both persisted `last_error` and stderr evidence
  - `CdcE2eHarness` already captures runner stderr snapshots, so the test support boundary is present

## Boundary Decision

- Keep formatting and actual stderr writes at the process boundary.
  - Do not add direct `eprintln!` calls inside reconcile apply logic.
- Introduce one honest runtime operator-event path for bounded reconcile failures.
  - The reconcile runtime should emit a typed operator event at the moment the apply failure is recorded.
  - `main.rs` should write emitted events for both text and JSON formats instead of silently discarding text-mode events.
- Preserve persistence and operator-surface concerns as separate responsibilities:
  - `tracking_state` owns durable failure recording
  - reconcile runtime owns deciding when an operator event should be emitted
  - process entrypoint owns formatting that event to stderr

## Improve-Code-Boundaries Focus

- Primary smell:
  - runtime event emission exists, but text-mode delivery is owned incorrectly in `main.rs`, and reconcile failure reporting is split between durable state updates and missing operator-surface emission
- Required cleanup during execution:
  - remove the `if log_format.writes_json()` gate around callback event delivery in `crates/runner/src/main.rs`
  - add one narrow runner-owned event constructor or helper for reconcile apply failures so tests and runtime share one semantic message owner
  - avoid duplicating reconcile failure strings across long-lane tests, audit helpers, and runtime code
- Preferred cleanup shape:
  - one typed/logical place defines the reconcile failure operator message
  - `main.rs` always writes emitted events using the selected `LogFormat`
  - runtime modules emit events, but never format stderr directly
- Bold refactor allowance:
  - if the current `emit_event` closure threading is too awkward, introduce a small runner-local event sink abstraction rather than smuggling more formatting decisions through unrelated modules

## Intended Public Contract

- During `runner run` in default text mode, a reconcile apply failure for a selected table must emit stderr text while the process remains alive.
- The stderr event must mention:
  - reconcile apply failure
  - mapping id
  - real table name
  - reconcile phase (`upsert` or `delete`)
  - database error detail
- JSON mode must still emit structured stderr events for the same failure path.
- Persisted `last_error` must still include table-specific reconcile failure context.
- The schema-mismatch path must remain bounded:
  - latest received watermark advances
  - last reconciled watermark stays at the last good checkpoint
  - helper state reflects the new row
  - ingress does not amplify retries

## Files And Structure To Add Or Change

- `crates/runner/tests/default_bootstrap_long_lane.rs`
  - keep or sharpen the RED long-lane assertion that proves stderr is missing today
- `crates/runner/tests/support/e2e_integrity.rs`
  - keep the schema-mismatch audit assertion aligned with the real operator contract
- `crates/runner/src/main.rs`
  - write emitted runtime events to stderr in both text and JSON modes
- `crates/runner/src/lib.rs`
  - thread the runtime event sink honestly through `run` execution as needed
- `crates/runner/src/reconcile_runtime/mod.rs`
  - emit a typed operator event when a reconcile apply failure is persisted
- `crates/runner/src/error.rs`
  - update only if a typed reconcile-failure event helper naturally belongs with existing error context
- `crates/operator-log/src/lib.rs`
  - change only if execution reveals a missing capability for stable text/json parity; avoid expanding this crate unless necessary

## Vertical TDD Slices

### Slice 1: Tracer Bullet For The Missing Operator Signal

- RED:
  - run the smallest existing schema-mismatch coverage that proves the current text-mode runner persists `last_error` but does not emit stderr context
  - prefer the existing long-lane schema-mismatch assertion already added in `default_bootstrap_long_lane.rs`
  - if that test is too broad to iterate honestly, carve out one narrower integration contract around the same public runtime path without mocking internals
- GREEN:
  - make that first failure pass with the smallest honest event-emission change
- REFACTOR:
  - keep the failure assertion tied to observable stderr output, not internal function calls

### Slice 2: Restore Text-Mode Runtime Event Delivery

- RED:
  - add or keep a failing assertion that emitted runtime events are lost in text mode
- GREEN:
  - remove the text-mode callback suppression in `main.rs` so emitted events always reach stderr using the chosen format
- REFACTOR:
  - keep `main.rs` as a thin formatter/writer boundary, not a policy owner for which runtime events matter

### Slice 3: Emit A Reconcile Apply Failure Event At The Correct Boundary

- RED:
  - add the next failing coverage that proves a bounded reconcile apply failure still lacks a live operator event even after text-mode event delivery is restored
- GREEN:
  - emit one typed operator event from `reconcile_runtime::run_reconcile_pass` after rollback and failure-state persistence
  - include mapping id, database, table, phase, and error detail
- REFACTOR:
  - centralize the operator message/event construction instead of scattering raw string assembly

### Slice 4: Prove Persisted State And Operator Logs Stay In Sync

- RED:
  - extend the next failing assertion so the schema-mismatch audit requires both:
    - persisted `last_error`
    - stderr evidence for the same failure
- GREEN:
  - adjust runtime event content or audit helpers until both signals reflect the same failure honestly
- REFACTOR:
  - remove duplicate phrase ownership if tests and runtime are asserting against slightly different ad hoc wording

### Slice 5: Prove No Retry Amplification Regression

- RED:
  - keep or add a failing assertion that ingress attempt count remains one while the failure is happening
- GREEN:
  - confirm the logging change does not alter the bounded reconcile behavior
- REFACTOR:
  - keep delivery-count checks owned by existing schema-mismatch audit helpers rather than re-implementing them in a second harness path

### Slice 6: Final Boundary Pass And Required Lanes

- RED:
  - after behavior slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - for the long-lane proof required by this bug, run the narrowest honest ignored schema-mismatch lane that demonstrates stderr plus persisted `last_error`
  - only escalate to `make test-long` if the task cannot be honestly completed without the long/e2e lane, since normal-task default is still to avoid it
- GREEN:
  - continue until all required default lanes pass and the long-lane schema-mismatch proof is satisfied
- REFACTOR:
  - do one final `improve-code-boundaries` pass so event ownership, persistence ownership, and stderr formatting ownership are separated cleanly

## TDD Guardrails For Execution

- One failing test slice at a time.
- Do not write tests after the implementation for the same behavior.
- Do not mock internal reconcile collaborators just to inspect event emission.
- Do not make the runner exit on reconcile apply failure; the bug is missing observability, not missing process termination.
- Do not add direct stderr writes inside persistence helpers or low-level reconcile apply functions.
- Do not weaken the long-lane audit by removing the stderr assertion.
- Do not ignore any errors encountered while wiring event emission; if the current design cannot express the event honestly, switch back to `TO BE VERIFIED`.

## Final Verification For The Execution Turn

- [x] Existing RED/GREEN slices are checked off in the plan as they complete
- [x] The schema-mismatch runtime emits operator-visible stderr context while staying alive
- [x] Persisted `last_error` still records the reconcile failure
- [x] Ingress delivery count stays bounded without retry amplification
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] Run only the necessary ignored long-lane proof for this bug, and use `make test-long` only if the task truly requires the broader lane
- [x] Final `improve-code-boundaries` pass confirms event emission, persistence, and formatting live at the right boundaries
- [x] Update the task file checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/bugs/bug-schema-mismatch-reconcile-failure-missing-operator-stderr_plans/2026-04-20-reconcile-failure-stderr-plan.md`

NOW EXECUTE
