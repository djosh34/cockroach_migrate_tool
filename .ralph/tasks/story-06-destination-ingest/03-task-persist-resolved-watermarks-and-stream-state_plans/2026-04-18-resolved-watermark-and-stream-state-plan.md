# Plan: Persist Resolved Watermarks And Stream Tracking State

## References

- Task: `.ralph/tasks/story-06-destination-ingest/03-task-persist-resolved-watermarks-and-stream-state.md`
- Previous task plan: `.ralph/tasks/story-06-destination-ingest/02-task-persist-row-batches-into-helper-shadow-tables_plans/2026-04-18-row-batch-helper-persistence-plan.md`
- Design: `designs/crdb-to-postgres-cdc/03_shadow_table_architecture.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Current implementation: `crates/runner/src/postgres_bootstrap.rs`
- Current implementation: `crates/runner/src/helper_plan.rs`
- Current implementation: `crates/runner/src/webhook_runtime/mod.rs`
- Current implementation: `crates/runner/src/webhook_runtime/routing.rs`
- Current implementation: `crates/runner/src/webhook_runtime/persistence.rs`
- Current tests: `crates/runner/tests/bootstrap_contract.rs`
- Current tests: `crates/runner/tests/webhook_contract.rs`
- Current tests: `crates/runner/tests/long_lane.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The public ingest surface from task 01 remains the contract:
  - `POST /ingest/:mapping_id`
  - row batches still persist helper-shadow rows
  - resolved messages become real tracked success paths in this task
- This task owns persisted ingest-side checkpoint state only. It must not implement reconcile logic or advance `latest_reconciled_resolved_watermark` beyond keeping the placeholder field durable and queryable.
- The runtime currently knows `mapping_id`, configured `source_database`, destination connection details, and helper-table metadata. It does not receive `source_job_id` or `starting_cursor` from the live webhook contract yet, so this task should preserve those columns as nullable placeholders instead of inventing a new source-metadata ingestion interface.
- Restart safety matters more than one-time bootstrap convenience. Startup must not wipe previously stored resolved or sync-state progress when the runner is restarted.
- If the first execution slices prove that resolved-watermark monotonicity cannot be expressed safely with the current `TEXT` watermark shape, or that this task needs a new source-bootstrap handoff contract for `job_id` and `starting_cursor`, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Keep the HTTP interface unchanged and make the resolved branch honest:
  - valid resolved requests for known mappings return `200 OK`
  - `200` is emitted only after the tracking-state transaction commits
  - failed tracking persistence returns non-`200` so Cockroach retries
- Replace the current split bootstrap/runtime ownership with one canonical mapping runtime shape. Today bootstrap returns only helper-table metadata, then runtime rebuilds tracking facts from config again. Task 03 should flatten that into one shared mapping state that carries:
  - mapping id
  - source database
  - destination connection
  - helper-table plans
  - stream-state seed facts
  - table-sync seed facts
- Keep row persistence and tracking persistence as separate deep modules under `webhook_runtime`:
  - row-batch persistence continues to own helper-shadow row SQL
  - a new tracking-state module owns `stream_state` and `table_sync_state` reads and writes
  - HTTP handlers and dispatch should only choose the typed action, never assemble SQL
- Bootstrap must seed tracking rows idempotently:
  - `stream_state`: one row per mapping
  - `table_sync_state`: one row per mapped table
  - reruns preserve existing progress columns instead of resetting them
- Resolved updates should be monotonic for `latest_received_resolved_watermark`. Duplicate or older resolved deliveries must not move the checkpoint backward.

## Public Contract To Establish

- The destination helper schema contains durable queryable tracking rows after startup:
  - `stream_state` has one row per mapping
  - `table_sync_state` has one row per mapped source table with the selected helper-table name
- A valid resolved message for a known mapping updates `stream_state.latest_received_resolved_watermark` and returns `200 OK` only after commit.
- Invalid resolved payloads still return `400`, unknown mappings still return `404`, and persistence failures still return `500`.
- Restarting the runner preserves previously stored tracking progress:
  - existing `latest_received_resolved_watermark` survives restart
  - existing `table_sync_state` progress fields survive restart
  - startup may refresh canonical metadata such as `source_database` and `helper_table_name`, but must not erase progress
- Multi-mapping startup keeps each destination database isolated:
  - each mapping seeds and updates only its own destination database
  - resolved updates for `app-a` never touch `app-b`

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this task:
  - bootstrap already knows the authoritative mapping-to-destination and mapping-to-helper-table facts, but runtime resolved handling currently throws that away and treats tracking state as an untyped special case
- Required cleanup from that:
  - promote bootstrap output from `MappingHelperPlan`-only data into a canonical mapping runtime/tracking plan reused by webhook routing and tracking persistence
  - seed `stream_state` and `table_sync_state` from that same plan during bootstrap instead of rediscovering tables or helper names later
  - remove the raw `DispatchTarget::Resolved { mapping_id, resolved }` shape in favor of one typed tracking action that already knows its destination and tracking seed metadata
- Secondary cleanup:
  - keep progress-row merge rules in one module so bootstrap reruns and resolved updates cannot drift
  - avoid stringly table-name rebuilding when table-sync seeds can reuse helper-plan types directly
  - keep nullable placeholder handling for `source_job_id`, `starting_cursor`, and reconciled watermark fields in one explicit state layer

## Files And Structure To Add Or Change

- [x] `crates/runner/src/postgres_bootstrap.rs`
  - seed `stream_state` and `table_sync_state` rows idempotently during startup without erasing existing progress
- [x] `crates/runner/src/helper_plan.rs`
  - existing helper-table metadata was already sufficient; no source change was needed after the boundary review
- [x] `crates/runner/src/webhook_runtime/routing.rs`
  - replace the current raw resolved dispatch output with a typed tracking action built from the canonical runtime plan
- [x] `crates/runner/src/webhook_runtime/mod.rs`
  - dispatch resolved requests into real tracking persistence and keep `200` semantics commit-bound
- [x] `crates/runner/src/webhook_runtime/persistence.rs`
  - row-batch persistence stayed focused on helper-shadow row mutations; no tracking SQL was added there
- [x] `crates/runner/src/webhook_runtime/tracking.rs`
  - new module that owns stream-state and table-sync persistence, restart-safe seed merges, and resolved-watermark updates
- [x] `crates/runner/src/error.rs`
  - add typed tracking-state bootstrap/read/write failures and remove the fake resolved-not-implemented path
- [x] `crates/runner/tests/bootstrap_contract.rs`
  - add startup coverage for seeded tracking rows and restart-safe preservation
- [x] `crates/runner/tests/webhook_contract.rs`
  - add real HTTPS coverage for resolved persistence, commit semantics, and restart behavior
- [x] `crates/runner/tests/long_lane.rs`
  - existing multi-mapping long-lane coverage remained sufficient; `make test-long` passed without source changes

## TDD Execution Order

### Slice 1: Tracer Bullet For Seeded Stream State

- [x] RED: add one failing bootstrap contract test that starts the runner and expects a `stream_state` row plus `table_sync_state` rows to exist for the configured mapping
- [x] GREEN: implement the minimal bootstrap seeding path so startup inserts the tracking rows without changing the public runtime contract
- [x] REFACTOR: move seed-row rendering and metadata shaping behind a dedicated tracking-state boundary instead of scattering SQL in bootstrap

### Slice 2: Resolved Requests Persist Before `200`

- [x] RED: add one failing HTTPS contract test that posts a resolved message and verifies the response is `200` only when `stream_state.latest_received_resolved_watermark` was durably updated
- [x] GREEN: implement the minimal resolved persistence path and remove `ResolvedNotImplemented`
- [x] REFACTOR: make dispatch carry a typed resolved-tracking action rather than naked `mapping_id` and `resolved` strings

### Slice 3: Restart Preserves Existing Progress

- [x] RED: add failing coverage that starts the runner, persists a resolved watermark, restarts the runner, and proves the stored watermark still exists after bootstrap reruns
- [x] GREEN: change startup seeding to preserve existing progress columns instead of overwriting them with defaults
- [x] REFACTOR: centralize seed-merge rules so bootstrap and runtime state transitions cannot diverge

### Slice 4: Table Sync Rows Stay Stable And Queryable

- [x] RED: add failing coverage that expects one `table_sync_state` row per mapped table with stable `helper_table_name` values and untouched progress placeholders across restart
- [x] GREEN: seed per-table tracking rows from helper-plan metadata and preserve existing sync progress fields on rerun
- [x] REFACTOR: remove duplicate helper-table-name construction outside the canonical helper-plan path

### Slice 5: Multi-Mapping Isolation

- [x] RED: add failing coverage for two mappings across two destination databases and prove each database receives only its own tracking rows and resolved updates
- [x] GREEN: ensure the canonical runtime/tracking plan keeps mapping-specific destination state isolated
- [x] REFACTOR: flatten any duplicated bootstrap/runtime mapping assembly that still rebuilds destination facts twice

### Slice 6: Monotonic Resolved Watermark Updates

- [x] RED: add failing coverage that replays an older or duplicate resolved watermark after a newer one and expects `latest_received_resolved_watermark` not to regress
- [x] GREEN: implement monotonic update rules for `latest_received_resolved_watermark`
- [x] REFACTOR: keep watermark comparison and update logic in the tracking module so future reconcile work can reuse the same checkpoint semantics

### Slice 7: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to remove leftover split ownership between bootstrap tracking seeds and runtime tracking updates

## Boundary Review Checklist

- [x] No resolved `200` is emitted before the tracking-state transaction commits
- [x] No bootstrap rerun wipes stored ingest or sync progress
- [x] No SQL rendering for tracking tables lives in the HTTP handler
- [x] No duplicate mapping-to-destination tracking metadata path exists across bootstrap and runtime
- [x] No table-sync helper-table naming is rebuilt outside canonical helper-plan metadata
- [x] No tracking persistence failure is swallowed, downgraded, or converted into fake success
- [x] No new source-bootstrap handoff contract is invented without explicitly switching back to `TO BE VERIFIED`

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
