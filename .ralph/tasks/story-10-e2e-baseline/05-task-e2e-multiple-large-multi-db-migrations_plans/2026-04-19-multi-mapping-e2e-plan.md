# Plan: End-To-End Multiple Large Multi-Database Migrations Under One Container

## References

- Task: `.ralph/tasks/story-10-e2e-baseline/05-task-e2e-multiple-large-multi-db-migrations.md`
- Previous story-10 plan: `.ralph/tasks/story-10-e2e-baseline/04-task-e2e-composite-pk-and-excluded-table-handling_plans/2026-04-19-composite-pk-excluded-table-e2e-plan.md`
- Existing long-lane scenarios:
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Existing long-lane support:
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/composite_pk_exclusion_harness.rs`
- Existing shared-mapping contract evidence:
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/source-bootstrap/tests/bootstrap_contract.rs`
- Design:
  - `designs/crdb-to-postgres-cdc/02_requirements.md`
  - `designs/crdb-to-postgres-cdc/06_recommended_design.md`
  - `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- The shipped runtime already intends to support one runner process managing multiple mappings, each with its own source database and destination database, while sharing one PostgreSQL container host.
- This task should prove the production-shaped contract, not invent a new one:
  - one `runner run --config <path>` process
  - one `source-bootstrap render-bootstrap-script --config <path>` script spanning multiple mappings
  - one Cockroach container hosting multiple source databases
  - one Postgres container hosting multiple destination databases
  - one HTTPS webhook runtime serving all mapping endpoints
- The long lane must cover at least two mappings with materially different table shapes so the scenario is not just a duplicated toy table:
  - one mapping should include a composite-PK or multi-table shape
  - the other should include a separate table family so cross-talk bugs are observable
- The scenario must prove both initial scan and live catch-up after CDC setup. After bootstrap completes, no more raw source-side scripting is allowed beyond the planned application-like writes executed through the existing harness interface.
- Verification must remain on the public CLI boundary by running `runner verify` separately per mapping against the real target tables in each destination database.
- If the first RED slice proves the runtime cannot bootstrap or verify multiple mappings with one config without changing the public config contract, this plan must switch back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- The task markdown plus the already-shipped multi-mapping contract tests are treated as approval for the public interface and required behaviors.
- Highest-priority behaviors to prove:
  - one runner process can own two mappings at the same time
  - one source-bootstrap script can create one changefeed per mapping from one start cursor
  - helper shadow state stays local to each destination database
  - live writes in source database `demo_a` never leak into destination database `app_b`
  - live writes in source database `demo_b` never leak into destination database `app_a`
  - `runner verify` succeeds for each mapping and mentions only that mapping's real selected tables

## Interface And Boundary Decisions

- Keep all product CLIs unchanged.
- Keep the multi-mapping contract expressed through one config file containing multiple `mappings`.
- Do not overload `CdcE2eHarness` into a sprawling union of single-mapping and multi-mapping code paths.
- Treat the main boundary problem as test-support ownership:
  - `crates/runner/tests/support/e2e_harness.rs` is honest for one mapping, one destination database, and one fixed MOLT target rewrite
  - task 05 needs a dedicated multi-mapping support boundary so the long-lane file stays behavioral instead of becoming a second config renderer plus database router
- Preferred support split:
  - keep `CdcE2eHarness` for the single-mapping scenarios that already use it
  - add a new dedicated support module for the multi-mapping long lane
  - extract only truly generic environment or wrapper helpers out of `e2e_harness.rs` if both support boundaries need them
- Important boundary cleanup during execution:
  - the current MOLT wrapper in `e2e_harness.rs` hardcodes one destination database, which is wrong for this task's verify path
  - fix that at the helper boundary by making the wrapper honor the mapping-specific target URL instead of cloning fixed destination credentials into a script

## Public Contract To Establish

- One ignored long-lane scenario proves the runtime shape required in production:
  - source database `demo_a` migrates into destination database `app_a`
  - source database `demo_b` migrates into destination database `app_b`
  - both destination databases live inside one Postgres container
  - both mappings are managed by one runner process and one source-bootstrap script
- The scenario must prove all of the following:
  - initial scan lands both mappings into their own helper shadow tables and real destination tables
  - helper state for `app-a` is created only inside `app_a`
  - helper state for `app-b` is created only inside `app_b`
  - post-bootstrap writes to `demo_a` converge only in `app_a`
  - post-bootstrap writes to `demo_b` converge only in `app_b`
  - repeated reconcile passes keep both mappings stable
  - `runner verify` passes for `app-a`
  - `runner verify` passes for `app-b`
  - verify output for one mapping never mentions the other mapping's tables or helper tables

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - the current E2E support mixes three concerns that are only accidentally aligned in the single-mapping scenarios:
    - docker environment lifecycle
    - one-mapping config/materialization
    - one fixed-destination MOLT wrapper rewriting
- Required cleanup during execution:
  - introduce a dedicated multi-mapping scenario support module instead of growing `default_bootstrap_long_lane.rs` with inline YAML, SQL, and database-routing helpers
  - keep generic container and command helpers reusable, but keep mapping-shape decisions in scenario support
  - make MOLT target rewriting generic enough to preserve the mapping-specific destination database instead of hardcoding one database into the wrapper script
- Do not solve this by piling optional arrays and `Option<Vec<_>>` branches into `CdcE2eHarness`. If a new boundary is needed, add the new boundary honestly.

## Files And Structure To Add Or Change

- [x] `crates/runner/tests/default_bootstrap_long_lane.rs`
  - add one ignored multi-mapping long-lane scenario, or split the file if the added behavior makes the filename dishonest
- [x] `crates/runner/tests/support/multi_mapping_harness.rs`
  - preferred new support boundary for task 05
- [x] `crates/runner/tests/support/e2e_harness.rs`
  - extract only the generic helpers that both single-mapping and multi-mapping support really share
- [x] `crates/runner/tests/support/mod.rs`
  - export any shared support modules honestly if needed
- [x] Product code changes likely are not needed
  - probable hotspots only if the RED slices expose a real product bug:
    - `crates/runner/src/postgres_bootstrap.rs`
    - `crates/runner/src/reconcile_runtime/mod.rs`
    - `crates/runner/src/runtime_plan.rs`
    - `crates/runner/src/molt_verify/mod.rs`
    - `crates/source-bootstrap/src/render.rs`
- [x] No new public config fields should be added unless the RED slice proves the current `mappings` contract is insufficient

## Candidate Scenario Shape

- Mapping `app-a`
  - source database: `demo_a`
  - destination database: `app_a`
  - tables:
    - `public.customers`
    - `public.order_items`
- Mapping `app-b`
  - source database: `demo_b`
  - destination database: `app_b`
  - tables:
    - `public.invoices`
    - `public.invoice_lines`
- Why this shape:
  - `app-a` keeps the more complex composite-key shape
  - `app-b` adds a second independent table family so cross-database routing bugs are visible
  - both mappings are large enough to feel production-shaped without inventing unnecessary schema noise

## TDD Execution Order

### Slice 1: Tracer Bullet For Two Mappings Bootstrapping Through One Runner

- [x] RED: add one ignored failing long-lane scenario that starts one Cockroach container, one Postgres container, one runner process, and one source-bootstrap script containing two mappings, then asserts both destination databases receive their initial scan rows
- [x] GREEN: add only the minimum multi-mapping support needed to express the real bootstrap path
- [x] REFACTOR: move multi-mapping config writing and snapshot routing into dedicated support instead of leaving YAML and SQL assembly inside the test body

### Slice 2: Prove Helper Shadow State And Bootstrap Commands Stay Mapping-Scoped

- [x] RED: extend the scenario to assert:
  - helper tables in `app_a` mention only `app-a`
  - helper tables in `app_b` mention only `app-b`
  - the bootstrap wrapper log contains one rangefeed enable, one start-cursor capture, and one changefeed creation per mapping
- [x] GREEN: make only the harness changes needed for those real assertions to pass
- [x] REFACTOR: keep helper-table inventory and bootstrap-log inspection on reusable support boundaries rather than scattering string fragments through the test

### Slice 3: Prove Live Catch-Up Without Cross-Talk

- [x] RED: extend the same scenario with post-bootstrap source writes in both `demo_a` and `demo_b`, then assert:
  - `app_a` converges only to the `demo_a` writes
  - `app_b` converges only to the `demo_b` writes
  - no rows appear in the wrong destination database
- [x] GREEN: fix only the first real runtime or harness gap the scenario exposes
- [x] REFACTOR: keep per-mapping snapshots and mutation helpers in scenario support so the test reads as behavior, not as SQL plumbing

### Slice 4: Real Verify Coverage For Both Destination Databases

- [x] RED: run `runner verify` for `app-a` and `app-b`, asserting matched verdicts and mapping-specific table lists only
- [x] GREEN: adjust only the real code or wrapper support needed for the public verify command to work across both destination databases
- [x] REFACTOR: make the MOLT wrapper boundary honest and generic rather than duplicating one wrapper per mapping without need

### Slice 5: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`, fixing only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass so the multi-mapping support is honest and single-mapping support did not regress into a generic mess

## TDD Guardrails For Execution

- Every new behavior assertion must fail before code changes are added. If a proposed assertion already passes, replace it with the next uncovered behavior.
- Do not fake webhook payloads, helper-table contents, or verify output in the final ignored long-lane scenario.
- Do not add extra raw source commands after CDC setup beyond the planned writes that model application activity in `demo_a` and `demo_b`.
- Do not weaken the task by collapsing both mappings into one shared destination database. The contract here is one container with multiple destination databases.
- Do not silently ignore bootstrap, runner, Docker, or verify failures. Any such failure is a real task failure.
- If the multi-mapping support file becomes just a thin alias over a more generic environment helper, flatten or rename the boundary so ownership remains honest.

## Boundary Review Checklist

- [x] The long-lane test reads as operator behavior, not as inline config and SQL plumbing
- [x] Single-mapping support remains simple instead of absorbing multi-mapping branches
- [x] Multi-mapping config generation lives behind a dedicated support boundary
- [x] MOLT target rewriting is generic enough to preserve mapping-specific destination databases
- [x] Helper-state assertions are explicitly per destination database
- [x] No real-path error is swallowed

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
