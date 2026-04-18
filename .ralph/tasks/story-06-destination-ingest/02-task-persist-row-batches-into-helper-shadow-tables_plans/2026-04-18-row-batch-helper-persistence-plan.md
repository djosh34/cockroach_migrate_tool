# Plan: Persist Row Batches Idempotently Into Helper Shadow Tables

## References

- Task: `.ralph/tasks/story-06-destination-ingest/02-task-persist-row-batches-into-helper-shadow-tables.md`
- Previous task plan: `.ralph/tasks/story-06-destination-ingest/01-task-build-https-webhook-server-and-routing_plans/2026-04-18-https-webhook-server-and-routing-plan.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Current implementation: `crates/runner/src/webhook_runtime/mod.rs`
- Current implementation: `crates/runner/src/webhook_runtime/payload.rs`
- Current implementation: `crates/runner/src/webhook_runtime/routing.rs`
- Current implementation: `crates/runner/src/postgres_bootstrap.rs`
- Current implementation: `crates/runner/src/helper_plan.rs`
- Current tests: `crates/runner/tests/webhook_contract.rs`
- Current tests: `crates/runner/tests/bootstrap_contract.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The task markdown plus the task-01 HTTPS routing contract are treated as approval for the public interface in this plan.
- Task 02 changes only row-batch semantics. Resolved-watermark requests must remain on the honest unimplemented path until task 03 owns them.
- Webhook success must mean one PostgreSQL transaction committed durable helper-shadow state for the whole row batch. If any row in the batch fails, the request must not return `200` and partial helper writes must not survive.
- If the first execution slices prove that the webhook row-event shape needs a different typed boundary than planned here, or that the runtime must change the public HTTP contract to persist row batches correctly, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Keep the public HTTP surface from task 01:
  - `POST /ingest/:mapping_id`
  - row batches return `200` only after commit
  - resolved requests stay non-`200` in task 02
- Replace the current opaque dispatch boundary:
  - today `DispatchTarget::RowBatch` carries `Vec<RowEvent>` with raw JSON and the handler throws `PersistenceNotImplemented`
  - task 02 should promote row batches into one typed persistence-ready shape before dispatch, for example:
    - `RowMutationBatch`
    - `RowMutation`
    - `RowOperation`
    - `TableIngestPlan`
- The persistence boundary must own all SQL details. Handlers and routers should not assemble SQL strings or rediscover helper-table metadata.
- Reduce destination-connection facts once and reuse them in both bootstrap and ingest. Do not duplicate `PgConnectOptions::new().host(...).port(...).database(...)` in multiple modules.
- Reuse helper-plan metadata as the canonical source of:
  - helper table name
  - ordered helper columns
  - ordered primary-key columns
  - source-table to helper-table mapping
- Build one startup-owned runtime mapping state per configured mapping, for example:
  - route facts
  - destination connection options or pool
  - per-table helper persistence metadata
- Delete or flatten any duplicated bootstrap/runtime lookup path that rebuilds the same mapping-to-helper-table facts twice. This is the primary `improve-code-boundaries` target for the execution turn.

## Public Contract To Establish

- A valid row batch for a mapped table returns `200 OK` only after helper-shadow persistence commits.
- Insert-like and update-like row events upsert helper-shadow rows by primary key.
- Delete row events remove helper-shadow rows by primary key.
- Duplicate delivery of the same row batch is safe:
  - replayed create/update events converge to the same helper state
  - replayed delete events leave the helper row absent
- Composite primary keys work without any special operator input.
- Any row-batch persistence failure returns a non-`200` status and leaves no partial helper-table changes from that request committed.
- Resolved-watermark requests remain routed but unimplemented in task 02 so task 03 can own checkpoint semantics explicitly.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this task:
  - startup/bootstrap already derives helper-table shape and primary-key metadata, but the runtime dispatch path currently throws that work away and would otherwise need to rediscover the same facts or build SQL ad hoc inside request handling
- Required cleanup from that:
  - move row-batch persistence into a dedicated persistence module under `webhook_runtime`
  - expose one canonical per-table ingest plan instead of passing raw table labels and raw JSON rows around
  - keep `lib.rs` and HTTP handlers thin
  - keep connection construction in one shared place
- Secondary cleanup:
  - stop creating table labels as string soup at the persistence boundary when a typed table/helper plan can carry both source and helper names
  - keep new persistence internals private unless another module truly needs them
  - prefer typed error variants over `format!` or `to_string()` buckets when mapping SQLx failures

## Files And Structure To Add Or Change

- [x] `crates/runner/src/webhook_runtime/mod.rs`
  - wire row-batch dispatch into the new persistence path and keep resolved handling on the existing unimplemented branch
- [x] `crates/runner/src/webhook_runtime/payload.rs`
  - parse row events into typed mutation data instead of leaving request handling on raw `serde_json::Value`
- [x] `crates/runner/src/webhook_runtime/routing.rs`
  - enrich routing output so persistence receives one selected table plan instead of a string table label plus opaque rows
- [x] `crates/runner/src/webhook_runtime/persistence.rs`
  - new module that owns PostgreSQL transaction handling and helper-table insert/update/delete behavior
- [x] `crates/runner/src/postgres_bootstrap.rs`
  - share or extract the mapping/helper-table planning work so startup and request-time persistence do not drift
- [x] `crates/runner/src/helper_plan.rs`
  - expose only the helper-plan metadata needed for runtime persistence from one canonical type
- [x] `crates/runner/src/config/mod.rs`
  - add one shared destination-connection reduction path if needed so bootstrap and persistence reuse the same connection shape
- [x] `crates/runner/src/error.rs`
  - add typed row-batch persistence and transaction-failure errors
- [x] `crates/runner/tests/webhook_contract.rs`
  - add contract coverage for helper persistence and commit semantics through the real HTTPS interface
- [x] `crates/runner/tests/long_lane.rs`
  - update only if the existing long lane needs fixture or expectation changes after the runtime refactor

## TDD Execution Order

### Slice 1: Tracer Bullet For Durable Insert

- [x] RED: add one failing HTTPS contract test that posts a single-row create event and then queries the helper shadow table to prove the row is not present before implementation
- [x] GREEN: implement the minimal row-batch persistence path so the request returns `200` and the helper shadow table contains the expected row after commit
- [x] REFACTOR: move any SQL assembly or helper-table lookup out of the handler into the persistence module

### Slice 2: Update Events Converge By Primary Key

- [x] RED: add failing contract coverage proving an update event for an existing helper row rewrites the stored values rather than inserting a duplicate row
- [x] GREEN: implement upsert behavior keyed by the helper-plan primary-key metadata
- [x] REFACTOR: centralize column assignment and conflict-target rendering so insert and update do not diverge

### Slice 3: Delete Events Remove Helper Rows

- [x] RED: add failing HTTPS coverage proving a delete event removes the helper row selected by primary key
- [x] GREEN: implement delete persistence using the same primary-key metadata path
- [x] REFACTOR: keep primary-key predicate rendering shared between upsert and delete planning

### Slice 4: Duplicate Delivery Is Safe

- [x] RED: add failing coverage that replays the same create/update/delete deliveries and expects the final helper state to remain correct
- [x] GREEN: make row-batch persistence idempotent through `INSERT .. ON CONFLICT DO UPDATE` plus keyed deletes
- [x] REFACTOR: remove any duplicate event-shape branching once the mutation planner owns operation-specific behavior

### Slice 5: Composite Primary Keys

- [x] RED: add failing contract coverage for a mapped table with a composite primary key and prove create/update/delete all target the correct helper row
- [x] GREEN: use ordered primary-key columns from the helper plan for conflict targets and delete predicates
- [x] REFACTOR: keep composite-key column ordering canonical in one helper-plan-derived path

### Slice 6: `200` Only After Commit

- [x] RED: add failing coverage for a multi-row batch where one row triggers a PostgreSQL error and assert both that the response is non-`200` and that no earlier row from the same batch was committed
- [x] GREEN: wrap each row batch in one PostgreSQL transaction and map commit/statement failures to a loud request error
- [x] REFACTOR: keep transaction management in the persistence module so HTTP code cannot accidentally acknowledge before commit

### Slice 7: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to remove any duplicate mapping/helper planning, raw-row plumbing, or connection-shape drift

## Boundary Review Checklist

- [x] No row-batch `200` is emitted before the database transaction commits
- [x] No SQL rendering lives in the HTTP handler
- [x] No duplicate destination connection builder exists across bootstrap and row-batch persistence
- [x] No duplicate helper-table metadata lookup path exists across bootstrap and ingest
- [x] No raw webhook row JSON is pushed deeper than the persistence-planning boundary when a typed mutation shape would do
- [x] No persistence failure is swallowed, downgraded, or converted into fake success
- [x] Resolved-watermark behavior remains explicitly out of scope for task 02

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
