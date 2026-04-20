# Plan: Audit Duplicate CDC, Recreated Feed Replay, And Schema Mismatch Full E2E Coverage

## References

- Task:
  - `.ralph/tasks/story-23-e2e-duplicate-cdc-and-schema-mismatch/01-task-audit-and-add-full-e2e-coverage-for-duplicate-cdc-recreation-and-schema-mismatch.md`
- Existing long-lane E2E surface:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
  - `crates/runner/tests/support/webhook_chaos_gateway.rs`
- Existing E2E integrity contracts:
  - `crates/runner/tests/e2e_integrity_contract.rs`
  - `crates/runner/tests/support/e2e_integrity_contract_support.rs`
- Relevant runtime behavior:
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/persistence.rs`
  - `crates/runner/src/reconcile_runtime/mod.rs`
  - `crates/runner/src/tracking_state.rs`
  - `crates/runner/src/error.rs`
- Existing design intent:
  - `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the public test surface and the highest-priority behaviors to prove in the next turn.
- This turn is planning-only because the task had no `<plan>` pointer and no prior execution marker.
- This task explicitly changes the full E2E long lane, so the execution turn must treat `make test-long` as required final validation instead of optional story-end extra credit.
- The current product should not be changed preemptively.
  - First prove the behavior with long-lane tests.
  - Only change runtime code if a failing test exposes a real defect or an unclear operator surface.
- If execution proves a dangerous retry pattern, hidden error, or unsafe correctness outcome:
  - create a bug immediately with `add-bug`
  - ask for a task switch
  - keep `<passes>false</passes>`

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - what actually happens when two Cockroach changefeeds push the same logical rows into the same destination URL at the same time
  - what actually happens when a feed is recreated with `initial_scan = 'yes'` and replays historical data
  - whether a source-destination schema mismatch becomes harmless, bounded-but-needs-operator-action, or defective
  - whether the runner logs and tracking state expose enough failure context for an operator
- Lower-priority concerns:
  - exact wording of every log line beyond the key operator-visible failure markers
  - extra chaos cases outside duplicate feeds, recreated feeds, and schema mismatch

## Current State Summary

- The existing long lane already proves some duplicate-related behavior, but only for transport-style retries:
  - `ignored_long_lane_retries_customer_update_after_external_http_500_and_converges`
  - `ignored_long_lane_recovers_after_helper_persistence_transaction_failure`
  - these confirm duplicate delivery caused by the same request being retried after an external failure
- The existing long lane already proves reconcile failure recording and recovery:
  - `ignored_long_lane_recovers_after_reconcile_transaction_failure`
  - this shows `last_error` persistence and clear reconcile failure context in stderr
- The existing long lane already proves heavy initial scan correctness:
  - `ignored_long_lane_handles_fk_heavy_initial_scan_and_live_catchup_into_real_postgres_tables`
- What is still missing or at least not explicit enough for this task:
  - no named long-lane scenario for two independently active changefeeds targeting the same sink
  - no named long-lane scenario for canceling a feed and recreating it with `initial_scan = 'yes'`
  - no named long-lane scenario for destination schema drift after bootstrap
  - no explicit scenario outcome recording that classifies each result as harmless, bounded-but-needs-operator-action, or defective
- The current harness already has the right raw ingredients:
  - it captures bootstrap source SQL and the explicit changefeed cursor
  - it can execute audited Cockroach SQL privately
  - it can observe gateway attempt counts and downstream status sequences
  - it can inspect helper-table state, real-table state, runner stderr, and durable tracking state
- The main missing piece is not raw capability.
  - It is the lack of a typed scenario owner for extra changefeed lifecycle and mismatch audits.

## Boundary Decision

- Primary boundary problem to flatten:
  - duplicate-feed creation, feed recreation, and schema-mismatch inspection would otherwise become scattered raw SQL, log scraping, and ad hoc assertions directly inside `default_bootstrap_long_lane.rs`
- Required cleanup during execution:
  - keep raw Cockroach changefeed lifecycle commands private to shared E2E support
  - expose scenario-specific methods from `DefaultBootstrapHarness` instead of letting tests issue raw SQL
  - record scenario conclusions through typed audits in `e2e_integrity.rs`, not loose booleans and string fragments spread across test bodies
- Preferred ownership split:
  - `default_bootstrap_harness.rs`
    - owns named scenario entrypoints the long lane calls
  - `e2e_harness.rs`
    - owns private raw Cockroach and PostgreSQL mechanics for extra feed lifecycle and low-level waits
  - `e2e_integrity.rs`
    - owns typed scenario audits and explicit conclusion classification
- Bold refactor allowance:
  - if existing helper methods in `e2e_harness.rs` are only supporting one scenario and fragment the workflow, inline or collapse them instead of adding more one-off helpers
  - if a new typed audit deletes several scattered string assertions, prefer the typed audit even if it removes older helper shapes

## Intended Public Test Contract

- The long-lane suite must explicitly answer these scenario questions:
  - concurrent duplicate feeds to one sink are harmless
  - or bounded but require operator action
  - or defective
- Recreated feed replay with `initial_scan = 'yes'` must explicitly answer the same outcome question.
- Schema mismatch must explicitly answer:
  - whether webhook ingest retries explode
  - whether reconcile failure is bounded and operator-visible
  - whether the runner logs and persisted error state explain the problem clearly
- The suite must prove behavior through the real path:
  - real Cockroach changefeeds
  - real HTTPS webhook ingress
  - real helper-table persistence
  - real reconcile into PostgreSQL
  - real verify-image correctness checks where correctness is the claim
- The shared harness must not gain a new public raw source SQL escape hatch for tests.

## Proposed Types And Scenario Shape

- Add a typed scenario outcome enum in `e2e_integrity.rs`, for example:
  - `ScenarioOutcome::Harmless`
  - `ScenarioOutcome::BoundedOperatorAction`
  - `ScenarioOutcome::Defective`
- Add typed audits that make the outcome explicit, for example:
  - `DuplicateFeedAudit`
  - `RecreatedFeedReplayAudit`
  - `SchemaMismatchAudit`
- Each audit should expose assertions that keep the test names honest:
  - duplicate helper state did not grow incorrectly
  - selected tables still match or explicitly mismatch
  - duplicate delivery or replay was observed when that is the point of the test
  - runner stderr or durable `last_error` contains operator-useful context
  - retry pressure stayed bounded enough for the chosen conclusion
- Do not introduce a second DTO layer just for symmetry.
  - Keep the audit structs shallow and directly useful to the long-lane tests.

## Files And Structure To Add Or Change

- `crates/runner/tests/default_bootstrap_long_lane.rs`
  - add the missing named long-lane scenarios
- `crates/runner/tests/support/default_bootstrap_harness.rs`
  - add named scenario helpers for duplicate feeds, recreated feeds, and schema mismatch
- `crates/runner/tests/support/e2e_harness.rs`
  - add the private mechanics needed to:
    - capture or recreate changefeed SQL intentionally
    - create a second changefeed to the same sink
    - stop an existing changefeed job
    - recreate a feed with `initial_scan = 'yes'`
    - mutate destination schema after bootstrap
    - wait on bounded status or tracking evidence without exposing raw SQL publicly
- `crates/runner/tests/support/e2e_integrity.rs`
  - add typed scenario audits and conclusion classification
- `crates/runner/tests/e2e_integrity_contract.rs`
  - add contract coverage that the long lane owns these scenarios through typed support instead of ad hoc escapes
- Runtime source files only if a test proves a real product defect:
  - `crates/runner/src/webhook_runtime/*`
  - `crates/runner/src/reconcile_runtime/*`
  - `crates/runner/src/tracking_state.rs`
  - `crates/runner/src/error.rs`

## Vertical TDD Slices

### Slice 1: Tracer Bullet For Typed Scenario Ownership

- RED:
  - add a failing integrity contract that requires:
    - explicit named long-lane scenarios for duplicate feeds, recreated feed replay, and schema mismatch
    - typed scenario audits in `e2e_integrity.rs`
    - no new public raw source SQL helper on the shared harness
- GREEN:
  - add the smallest typed audit skeleton and named harness entrypoints needed to satisfy the contract
- REFACTOR:
  - keep the ownership honest before adding the heavy long-lane behavior tests

### Slice 2: Audit And Prove Concurrent Duplicate Feed Delivery

- RED:
  - add a new ignored long-lane test that:
    - bootstraps the default migration
    - starts a second Cockroach changefeed for the same source table set and the same sink URL
    - applies live source updates that both feeds will deliver
    - proves duplicate deliveries are actually observed
    - proves helper and real tables converge to the correct net state
    - classifies the scenario outcome explicitly
- GREEN:
  - add only the support needed to create the second feed and capture the resulting audit
- REFACTOR:
  - keep extra changefeed lifecycle hidden behind named support methods
- Stop condition:
  - if correctness breaks, helper state grows incorrectly, or duplicate feeds create unsafe behavior, create a bug and ask for a task switch

### Slice 3: Audit And Prove Recreated Feed Replay With `initial_scan = 'yes'`

- RED:
  - add a new ignored long-lane test that:
    - bootstraps normally
    - stops the original changefeed
    - recreates it with `initial_scan = 'yes'`
    - observes historical replay into the same destination path
    - proves the helper plus reconcile design either safely absorbs replay or exposes a defect
    - classifies the scenario outcome explicitly
- GREEN:
  - add the smallest support needed to stop and recreate changefeeds intentionally
- REFACTOR:
  - if the support can reuse one typed changefeed lifecycle path for both slice 2 and slice 3, collapse to that one owner
- Stop condition:
  - if replay is not idempotent or produces unsafe growth/correctness issues, create a bug and ask for a task switch

### Slice 4: Audit And Prove Schema Mismatch Failure Mode

- RED:
  - add a new ignored long-lane test that:
    - bootstraps normally
    - mutates the real destination table schema into an incompatible shape after bootstrap
    - applies a live source change
    - proves whether ingress keeps returning success while reconcile records a bounded failure, or whether retries amplify dangerously
    - proves runner stderr and durable `last_error` expose enough operator-visible context
    - classifies the scenario outcome explicitly
- GREEN:
  - add the minimum support needed to create the mismatch and collect the failure audit
- REFACTOR:
  - keep schema-mismatch evidence in a typed audit instead of raw stderr fragments duplicated in the test
- Stop condition:
  - if schema mismatch causes dangerous retry amplification or hidden failure, create a bug and ask for a task switch

### Slice 5: Final Audit Consolidation And Long-Lane Gate

- RED:
  - once the three scenario slices are green, tighten the integrity contract so the scenario coverage is named and cannot silently disappear
  - run the required repository lanes and let failures drive the remaining fixes:
    - `make check`
    - `make lint`
    - `make test`
    - `make test-long`
- GREEN:
  - continue until all four lanes pass
- REFACTOR:
  - do one final `improve-code-boundaries` pass focused on:
    - no raw-sql scenario logic leaking into the long-lane file
    - no duplicate scenario-classification logic spread across test files
    - no tiny one-caller support helpers pretending to be reusable abstractions

## Coverage Conclusions To Record During Execution

- Concurrent duplicate feeds:
  - the test name plus audit assertion must state whether the behavior is harmless, bounded-but-needs-operator-action, or defective
- Recreated feed replay:
  - the test name plus audit assertion must state the same conclusion explicitly
- Schema mismatch:
  - the test name plus audit assertion must state whether the failure is bounded-and-operator-usable or defective
- If any scenario is defective:
  - file a bug immediately
  - do not mark the task passed

## Verification Plan

- Execute strictly as vertical TDD.
  - one new failing contract or long-lane scenario at a time
  - minimum support to go green
  - immediate refactor
- Required end-of-task lanes for the execution turn:
  - `make check`
  - `make lint`
  - `make test`
  - `make test-long`
- Final `improve-code-boundaries` pass:
  - scenario lifecycle logic has one honest owner
  - scenario conclusions are typed and explicit
  - no new raw source SQL public escape hatch exists
  - helper layering is flatter than before, not muddier

Plan path: `.ralph/tasks/story-23-e2e-duplicate-cdc-and-schema-mismatch/01-task-audit-and-add-full-e2e-coverage-for-duplicate-cdc-recreation-and-schema-mismatch_plans/2026-04-20-duplicate-cdc-schema-mismatch-e2e-plan.md`

NOW EXECUTE
