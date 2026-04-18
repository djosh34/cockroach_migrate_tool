# Plan: Run Multiple Database Mappings From One Destination Container

## References

- Task: `.ralph/tasks/story-08-multi-db-orchestration/01-task-run-multiple-db-mappings-from-one-destination-container.md`
- Previous task plan: `.ralph/tasks/story-07-reconcile/03-task-track-reconciled-watermarks-and-repeatable-sync-state_plans/2026-04-18-reconciled-watermarks-and-repeatable-sync-state-plan.md`
- Design: `designs/crdb-to-postgres-cdc/02_requirements.md`
- Design: `designs/crdb-to-postgres-cdc/03_shadow_table_architecture.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/05_design_decisions.md`
- Design: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Current implementation:
  - `crates/runner/src/config/mod.rs`
  - `crates/runner/src/postgres_bootstrap.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/tracking_state.rs`
  - `crates/runner/src/error.rs`
- Current tests:
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
  - `crates/runner/tests/long_lane.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The public runtime contract remains `runner run --config <path>`.
- Story 08 is not the place to add a second operator process, a central control database, or a second runtime binary.
- The config model already allows multiple mappings and currently also allows those mappings to reuse one destination database. Story 08 must make that runtime shape explicit and safe instead of leaving it as an accidental side effect.
- The existing suite already proves isolation across two different destination databases. The uncovered risk is the orchestration boundary when several mappings share one destination database or one destination connection contract.
- If the first execution slice shows that safe shared-database orchestration requires a materially different public contract than `runner run --config <path>`, or requires a second control-plane schema outside the destination database, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Introduce one canonical destination-ownership planning boundary for the runner:
  - group mappings by destination database identity before bootstrap and runtime startup
  - treat the destination database, not the individual mapping, as the resource boundary for helper schema ownership
  - keep webhook lookup by mapping id, but keep database-scoped orchestration derived from the grouped destination plan
- Make shared-database orchestration explicit and safe:
  - mappings may share one destination database only when they use one consistent destination connection contract
  - mappings inside one destination database must own disjoint destination table sets
  - if two mappings in one destination database claim the same real table, startup must fail loudly before serving traffic
- Keep helper state scoped correctly:
  - helper schema remains one schema per destination database
  - helper tables and tracking rows remain mapping-scoped inside that schema
  - stream state stays separate per mapping id even when two mappings share one destination database
- Keep runtime modules shallow:
  - one module owns destination grouping and duplicate-table validation
  - bootstrap consumes that grouped plan instead of rediscovering grouping ad hoc
  - reconcile worker startup consumes the same grouped plan instead of spawning only from a flat mapping list
  - webhook routing still resolves by mapping id and uses the canonical mapping plan produced from the grouped destination plan

## Public Contract To Establish

- One `runner run` process can own multiple source-to-destination mappings from one config file without collapsing helper state across mappings.
- Multiple mappings can safely target one shared destination database when:
  - they reuse the same destination connection contract
  - they own disjoint destination table sets
- When mappings share one destination database:
  - the helper schema is created once in that database
  - helper tables are still mapping-prefixed and mapping-specific
  - `_cockroach_migration_tool.stream_state` contains one row per mapping
  - `_cockroach_migration_tool.table_sync_state` contains one row per mapping plus source table
- Unsafe ownership is rejected early:
  - overlapping destination table ownership in one destination database fails at startup
  - inconsistent destination connection settings for the same destination database fail at startup if they would create ambiguous helper-schema ownership

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this task:
  - the current code treats each mapping as the orchestration unit everywhere
  - helper schema state and real-table ownership actually live at the destination-database boundary
  - this mismatch leaves shared-database safety implicit, duplicates grouping logic, and hides real ownership conflicts
- Required cleanup:
  - introduce one canonical destination-group plan instead of letting `postgres_bootstrap` and `runtime_plan` each reason from raw mappings
  - move shared-database validation out of the YAML parser and into the runner's orchestration planning boundary where cross-mapping semantics belong
  - expose grouped destination iteration for reconcile startup instead of making worker ownership an implementation accident of the flat mapping list
- Secondary cleanup:
  - keep per-mapping helper-table facts on `MappingRuntimePlan`
  - keep per-destination grouping facts on a separate destination-owned plan
  - do not duplicate destination grouping rules in tests, bootstrap, and runtime separately

## Files And Structure To Add Or Change

- [x] `crates/runner/src/runtime_plan.rs`
  - add one destination-group boundary that owns grouped mappings and exposes mapping lookup plus destination iteration
- [x] `crates/runner/src/postgres_bootstrap.rs`
  - bootstrap one destination database group at a time, running per-database scaffold once and per-mapping helper/tracking work inside that group
- [x] `crates/runner/src/reconcile_runtime/mod.rs`
  - start workers from the grouped destination plan rather than only from a flat mapping iterator
- [x] `crates/runner/src/config/mod.rs`
  - add only the minimal accessors or identifiers needed for destination grouping; keep parsing concerns separate
- [x] `crates/runner/src/error.rs`
  - add loud startup errors for unsafe shared-database ownership or inconsistent destination connection identity
- [x] `crates/runner/tests/bootstrap_contract.rs`
  - add shared-destination bootstrap coverage and startup failure coverage for unsafe ownership
- [x] `crates/runner/tests/webhook_contract.rs`
  - cover row-batch and resolved routing for two mappings that share one destination database without cross-talk
- [x] `crates/runner/tests/reconcile_contract.rs`
  - cover reconcile behavior for two mappings that share one destination database and prove real-table ownership stays separated

## TDD Execution Order

### Slice 1: Reject Unsafe Shared-Destination Table Ownership

- [x] RED: add one failing startup contract test with two mappings that share one destination database and both claim the same destination table, and assert `runner run` fails loudly before serving traffic
- [x] GREEN: implement canonical destination-group validation that rejects overlapping table ownership inside one destination database
- [x] REFACTOR: keep duplicate-table ownership checks in the new grouped destination boundary instead of scattering them through bootstrap and reconcile

### Slice 2: Reject Ambiguous Shared-Destination Connection Ownership

- [x] RED: add one failing startup contract test where two mappings target the same host/port/database but specify inconsistent destination connection identity, and assert startup fails loudly rather than silently choosing a helper-schema owner
- [x] GREEN: enforce one consistent destination connection contract for mappings that share one destination database
- [x] REFACTOR: reduce destination identity to one canonical key/value object reused by bootstrap and runtime

### Slice 3: Bootstrap One Shared Destination Database Safely

- [x] RED: add one failing bootstrap contract test for two mappings that share one destination database with disjoint table ownership, and assert one helper schema contains both mapping-specific helper tables plus separate stream/table tracking rows
- [x] GREEN: bootstrap per destination group so shared database scaffold is owned once and per-mapping helper/tracking state is still seeded separately
- [x] REFACTOR: remove any duplicated per-database DDL/setup path that survives after the grouped bootstrap boundary exists

### Slice 4: Route Webhook State Into The Correct Mapping Inside One Shared Destination Database

- [x] RED: add one failing webhook contract test for two mappings sharing one destination database, post row-batch and resolved payloads to both mapping paths, and assert helper rows plus stream state stay mapping-scoped in the same database
- [x] GREEN: drive dispatch from the canonical grouped runtime plan so shared-database routing uses the correct mapping-owned helper metadata
- [x] REFACTOR: keep webhook routing shallow and avoid rebuilding destination grouping logic in request handlers

### Slice 5: Reconcile Shared-Destination Mappings Without Cross-Talk

- [x] RED: add one failing reconcile contract test for two mappings sharing one destination database with disjoint tables, and assert each mapping reconciles only its own real tables while sharing the same helper schema database
- [x] GREEN: start reconcile work from the grouped destination plan and keep mapping application isolated within that destination database
- [x] REFACTOR: remove any leftover flat-mapping worker startup logic that no longer represents the real ownership boundary

### Slice 6: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm no duplicate destination-grouping or table-ownership logic remains

## TDD Guardrails For Execution

- Every planned test slice must fail before code is added. If a candidate shared-destination positive-path test already passes on the current code, do not keep it as a fake RED slice; replace it with the next uncovered failing behavior.
- Prefer integration-style tests through `runner run` and the HTTPS ingress path over unit tests of private helpers.
- Do not hide ownership conflicts behind warnings, skipped mappings, or best-effort behavior. Unsafe configuration must fail loudly.
- Do not swallow bootstrap, routing, or reconcile errors. If the chosen design makes a failure awkward to represent, add a typed error and keep the failure explicit.

## Boundary Review Checklist

- [x] No second runtime binary or second control database is introduced
- [x] No shared-destination ownership rule is left implicit
- [x] No overlapping destination-table ownership is allowed silently
- [x] No destination-grouping rule is duplicated across bootstrap and runtime
- [x] No webhook or reconcile path can update another mapping's stream state inside a shared destination database
- [x] No errors are downgraded into partial success or silent skips

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
