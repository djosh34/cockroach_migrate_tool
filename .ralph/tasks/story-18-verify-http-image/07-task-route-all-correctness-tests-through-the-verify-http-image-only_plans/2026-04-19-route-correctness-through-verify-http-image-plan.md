# Plan: Route All Correctness Tests Through The Verify HTTP Image Only

## References

- Task:
  - `.ralph/tasks/story-18-verify-http-image/07-task-route-all-correctness-tests-through-the-verify-http-image-only.md`
- Prior verify-image and HTTP-runtime tasks:
  - `.ralph/tasks/story-18-verify-http-image/03-task-build-a-scratch-verify-image-from-the-pruned-verify-source.md`
  - `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs.md`
  - `.ralph/tasks/story-18-verify-http-image/06-task-prove-http-request-inputs-cannot-cause-command-injection-in-verify-execution.md`
- Current verify image and verify-service runtime boundary:
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/runner/tests/support/verify_docker_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
- Current Rust correctness-test harnesses that still bypass the verify image:
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/composite_pk_exclusion_harness.rs`
  - `crates/runner/tests/support/multi_mapping_harness.rs`
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
  - `crates/runner/tests/e2e_integrity_contract.rs`
  - `crates/runner/tests/support/e2e_integrity_contract_support.rs`
- Skill:
  - `tdd`
- Skill:
  - `improve-code-boundaries`

## Planning Assumptions

- "Correctness verification" means proving source and destination data agree for the selected mapped tables.
  - That proof must happen through the supported production verify path only:
    - run the dedicated verify image
    - call its HTTP API
    - assert on the typed job result returned by `GET /jobs/{job_id}`
- Direct destination SQL snapshots may remain only for non-correctness concerns:
  - excluded-table assertions
  - helper-table inventory
  - tracking-state assertions
  - lock/failure probes
  - diagnostics when a correctness check fails
- No new runner-local verify shortcut is allowed.
  - No `runner verify`
  - No in-process Rust-side compare helper
  - No hidden direct-binary path used only by tests
- The verify-service config contract from task 04 stays intact.
  - Do not add insecure DB-mode escape hatches just to make tests easier.
  - Do not widen the HTTP request shape.
- The current Rust E2E harness still starts Cockroach with `--insecure` and writes source-bootstrap config with `sslmode=disable`.
  - The current verify-service config supports only TLS-backed source and destination connections.
  - If execution cannot establish an honest TLS-backed DB path for the verify image without adding a fake compatibility seam, execution must switch back to `TO BE VERIFIED` immediately.

## Current State Summary

- The verify image contract currently proves packaging only.
  - `VerifyImageHarness` builds the image and inspects entrypoint/filesystem/help output.
  - It does not start the runtime, submit jobs, or read job results.
- The runner E2E harnesses still prove correctness through direct SQL snapshots.
  - `CdcE2eHarness::wait_for_destination_query(...)`
  - `CdcE2eHarness::assert_destination_query_stable(...)`
  - `CdcE2eHarness::query_destination(...)`
  - scenario harness methods such as `wait_for_destination_customers(...)`
- Existing integrity contract tests only ban the old removed runner verify helpers.
  - They do not yet enforce that selected-table correctness must flow through the verify image HTTP surface.
- The Go verify-service already has the right public execution surface.
  - `POST /jobs`
  - `GET /jobs/{job_id}`
  - `POST /stop`
  - typed JSON result payload with `status_messages`, `summaries`, `mismatches`, and `errors`
- There is no health endpoint on the verify service.
  - Readiness for a real containerized runtime will need to be inferred from a harmless HTTP request, not from a new `/healthz` endpoint.

## Interface And Boundary Decisions

- Introduce one typed Rust-side correctness boundary.
  - Preferred owner: `crates/runner/tests/support/e2e_integrity.rs`
  - Preferred type name: `VerifyCorrectnessAudit`
- `VerifyCorrectnessAudit` should own only verify-result interpretation.
  - It should not start containers or write config files.
  - It should expose assertion helpers over typed job results, for example:
    - `assert_selected_tables_match()`
    - `assert_detects_table_mismatch(...)`
    - `assert_finished_successfully()`
- Extend `VerifyImageHarness` so it owns the verify-image runtime lifecycle as well as packaging.
  - build the image
  - create/remove its Docker network and runtime container
  - materialize config/cert mounts
  - submit `POST /jobs`
  - poll `GET /jobs/{job_id}`
  - stop/cleanup the active verify container
- Keep one owner for raw destination SQL and one owner for correctness verification.
  - `CdcE2eHarness` may keep raw SQL helpers for diagnostics and non-correctness assertions.
  - It should not remain the public owner of correctness assertions for selected mapped tables.
- Promote scenario harness APIs toward named correctness methods instead of raw snapshot methods.
  - Good:
    - `assert_selected_tables_match_via_verify_image()`
    - `wait_for_selected_tables_to_match_via_verify_image()`
  - Bad:
    - `wait_for_destination_customers(...)`
    - `assert_destination_customers_snapshot(...)`
    - `assert_destination_customers_stable(...)`
- Do not add a new verify-service health route just for tests.
  - Runtime readiness should be proven through the already-supported HTTP job surface.
  - A harmless `GET /jobs/nonexistent-probe` returning `404` is enough to prove the server is listening.

## Improve-Code-Boundaries Focus

- Primary smell: correctness logic currently lives in the wrong layer.
  - Scenario harnesses hand-author destination SQL snapshots and compare strings directly.
  - That bypasses the supported verify product boundary.
- Secondary smell: `VerifyImageHarness` is artificially shallow.
  - It owns packaging checks but not the runtime contract the image is supposed to provide.
  - The image contract should own image runtime behavior too.
- Tertiary smell: public bypass surface in `CdcE2eHarness`.
  - `query_destination` and snapshot-based wait/assert helpers make the cheating path easy to reintroduce.
  - Those helpers should become private or remain visible only for non-correctness diagnostics.
- Desired refactor shape:
  - one deep verify-image runtime harness
  - one typed correctness-audit layer
  - thinner scenario harnesses with named behavior methods
  - fewer raw snapshot strings escaping into long-lane test bodies

## Proposed Code Shape

- `crates/runner/tests/support/verify_image_harness.rs`
  - grow from packaging helper into a runtime harness
  - add runtime config materialization and typed HTTP helper methods
  - own Docker lifecycle for the verify container
- `crates/runner/tests/support/e2e_integrity.rs`
  - add `VerifyCorrectnessAudit`
  - add typed DTO parsing for verify job responses
  - keep assertion logic here, not in individual scenario harnesses
- `crates/runner/tests/support/e2e_harness.rs`
  - add the bridge from the shared E2E DB environment into verify-image runtime inputs
  - likely expose one narrow method that runs verify for the selected mapping and returns `VerifyCorrectnessAudit`
  - reduce visibility of raw correctness-bypass helpers
- `crates/runner/tests/support/default_bootstrap_harness.rs`
  - replace public customer-correctness snapshot assertions with verify-image-backed assertions
- `crates/runner/tests/support/composite_pk_exclusion_harness.rs`
  - keep excluded-table SQL assertions
  - move included-table correctness proof to verify-image-backed assertions
- `crates/runner/tests/support/multi_mapping_harness.rs`
  - route per-mapping correctness through verify-image-backed assertions instead of direct destination snapshots
- `crates/runner/tests/default_bootstrap_long_lane.rs`
  - switch correctness checkpoints to the typed verify-image audit API
- `crates/runner/tests/e2e_integrity_contract.rs`
  - add enforcement tests proving the suite no longer exposes raw selected-table correctness helpers
- `crates/runner/tests/support/e2e_integrity_contract_support.rs`
  - ban new bypass markers and verify-image omissions from the relevant harness files
- `crates/runner/tests/verify_image_contract.rs`
  - once runtime harness exists, add at least one real verify-image runtime contract test proving a job can be submitted and a result can be read through HTTP

## Resolved Execution Design

- The shared E2E environment should be upgraded, not bypassed.
  - Replace insecure Cockroach startup with a harness-owned secure fixture:
    - generate a temporary Cockroach CA, node cert, and `client.root` cert per harness run
    - start Cockroach with `--certs-dir`, not `--insecure`
    - keep SQL application and diagnostics using the same real secure cluster
  - Keep source-bootstrap rendering explicit, but stop hard-coding an insecure source URL in the harness-written config.
    - the rendered config should point at the secure Cockroach endpoint with TLS enabled
  - SSL-enable the shared Postgres fixture with a harness-generated server certificate signed by a temporary local CA
    - keep normal runner paths unchanged by allowing Postgres to continue accepting the existing non-SSL client path
    - have the verify image connect to Postgres through `verify-ca` using the generated CA file
- The verify-service product boundary stays strict.
  - Do not add `disable`, `require`, or other relaxed DB TLS modes to the verify-service config
  - Do not add test-only request fields or config escape hatches
- The verify image should connect to the same source and destination databases as the runner scenarios.
  - Source:
    - use the secure Cockroach container in the shared Docker network
    - supply the generated CA plus `client.root` cert/key to the verify image config
  - Destination:
    - use the same shared Postgres container and database state
    - supply only the generated CA to the verify image config unless Postgres auth requires more
- This keeps one correctness authority.
  - the runner still writes the destination state
  - the verify image becomes the only supported path that compares source and destination contents
  - no parallel fake correctness environment is introduced

## TDD Slices

### Slice 0: Honest Verify Connectivity Precondition

- [ ] RED: add one failing contract-level test that proves the verify image can reach the same source and destination databases used by the runner E2E harness through the real verify-service config contract
- [ ] GREEN: upgrade the shared test fixture to secure Cockroach plus SSL-capable Postgres, then mount the generated cert material into the verify image runtime
- [ ] REFACTOR: if this requires widening the verify-service config or adding insecure test-only flags, stop and switch back to `TO BE VERIFIED`

### Slice 1: Verify Image Runtime Harness

- [ ] RED: add one failing `verify_image_contract` test that starts the real verify image, submits `POST /jobs`, and reads the result through `GET /jobs/{job_id}`
- [ ] GREEN: extend `VerifyImageHarness` with runtime lifecycle, HTTP requests, readiness probe, and typed response decoding
- [ ] REFACTOR: keep all Docker/runtime details inside `VerifyImageHarness`; do not spread ad hoc `docker run` plus `reqwest` snippets across test files

### Slice 2: Typed Correctness Audit Boundary

- [ ] RED: add one failing test that proves verify job responses are interpreted only through a typed audit boundary, not by string-searching raw JSON in scenario harnesses
- [ ] GREEN: add `VerifyCorrectnessAudit` in `e2e_integrity.rs` and give it the smallest assertion API needed by the scenarios
- [ ] REFACTOR: keep typed verify-result interpretation in one place only

### Slice 3: Default Harness Migration

- [ ] RED: change one default-bootstrap long-lane scenario to call a new verify-image-backed correctness method and watch it fail
- [ ] GREEN: wire `DefaultBootstrapHarness` through the shared E2E environment into `VerifyImageHarness`, then delete the replaced direct snapshot correctness helper
- [ ] REFACTOR: keep helper-table and tracking assertions where they are, but remove selected-table correctness ownership from the scenario harness

### Slice 4: Composite-Key And Exclusion Harness Migration

- [ ] RED: change one composite-key scenario so included-table correctness must come from verify-image results while excluded-table assertions still use SQL
- [ ] GREEN: migrate the included-table correctness path and delete the replaced bypass helpers
- [ ] REFACTOR: preserve excluded-table/helper-table SQL assertions as explicit non-correctness checks

### Slice 5: Multi-Mapping Harness Migration

- [ ] RED: change one multi-mapping scenario so each mapping’s correctness is proven through the verify-image HTTP contract
- [ ] GREEN: add per-mapping verify-image-backed correctness helpers and delete direct selected-table snapshot correctness paths
- [ ] REFACTOR: keep mapping setup/state SQL helpers narrow and non-public where possible

### Slice 6: Bypass Enforcement Tests

- [ ] RED: add failing integrity-contract coverage that catches reintroduction of raw selected-table correctness helpers or missing verify-image correctness methods
- [ ] GREEN: update `e2e_integrity_contract.rs` and `e2e_integrity_contract_support.rs` so future bypasses fail loudly
- [ ] REFACTOR: keep the contract focused on bypass-prone surfaces, not every SQL helper in the repo

### Slice 7: Negative Proof That Verify Is The Real Correctness Authority

- [ ] RED: add one failing runtime test that deliberately introduces a selected-table mismatch and proves the verify image reports it
- [ ] GREEN: make the test pass through the real HTTP job result path only
- [ ] REFACTOR: do not add secondary compare helpers "for convenience"; let verify remain the only correctness authority

### Slice 8: Repository Validation Lanes

- [ ] RED: run `make check`, `make lint`, and `make test`, fixing only the first failing lane at a time
- [ ] GREEN: continue until all required lanes pass cleanly
- [ ] REFACTOR: do one final `improve-code-boundaries` pass so correctness ownership is flatter than it was before execution

## TDD Guardrails For Execution

- One failing test at a time.
- No horizontal slice of "all contract tests first, all harness work second".
- Prefer one tracer bullet through the real verify image before migrating every scenario harness.
- Do not add a fake in-process Rust comparison helper as an interim convenience.
- Do not reintroduce removed runner verify surfaces.
- Do not add insecure DB config compatibility to the verify service just to satisfy test infrastructure.
- Do not swallow verify-job failures.
  - failed or mismatching verify runs must stay explicit and typed
- If the first slice shows the TLS/runtime assumption is materially wrong, switch back to `TO BE VERIFIED` instead of patching around it.

## Boundary Review Checklist

- [ ] selected-table correctness is proven through the verify image HTTP contract only
- [ ] `VerifyImageHarness` owns the runtime container contract, not just packaging checks
- [ ] `VerifyCorrectnessAudit` owns verify-result interpretation in one typed place
- [ ] raw destination SQL remains only for diagnostics and non-correctness assertions
- [ ] scenario harnesses no longer expose snapshot-based correctness helpers for mapped tables
- [ ] integrity-contract tests fail if a bypass path or hidden alternate verification route is reintroduced
- [ ] no insecure verify-service DB compatibility path is added

## Final Verification For The Execution Turn

- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long`
  Not required unless execution changes the long-lane selection boundary or the task explicitly requires it
- [ ] final `improve-code-boundaries` pass after the required lanes are green
- [ ] update the task acceptance checkboxes and set `<passes>true</passes>` only after the required lanes pass

Plan path: `.ralph/tasks/story-18-verify-http-image/07-task-route-all-correctness-tests-through-the-verify-http-image-only_plans/2026-04-19-route-correctness-through-verify-http-image-plan.md`

NOW EXECUTE
