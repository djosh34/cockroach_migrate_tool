# Plan: End-To-End Default Database Bootstrap From Scratch

## References

- Task: `.ralph/tasks/story-10-e2e-baseline/01-task-e2e-default-database-bootstrap-from-scratch.md`
- Previous source-bootstrap plan: `.ralph/tasks/story-04-source-bootstrap/01-task-build-cockroach-bootstrap-command-and-script-output_plans/2026-04-18-cockroach-bootstrap-command-and-script-output-plan.md`
- Previous verification plan: `.ralph/tasks/story-09-verification-cutover/01-task-wrap-molt-verify-and-fail-on-log-detected-mismatches_plans/2026-04-19-molt-verify-wrapper-plan.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Investigation harness:
  - `investigations/cockroach-webhook-cdc/scripts/run.sh`
  - `investigations/cockroach-webhook-cdc/scripts/run-molt-verify.sh`
  - `investigations/cockroach-webhook-cdc/docker-compose.yml`
- Current implementation:
  - `crates/source-bootstrap/src/config/mod.rs`
  - `crates/source-bootstrap/src/config/parser.rs`
  - `crates/source-bootstrap/src/render.rs`
  - `crates/runner/src/lib.rs`
  - `crates/runner/src/molt_verify/mod.rs`
- Current tests:
  - `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
  - `crates/runner/tests/verify_contract.rs`
  - `crates/runner/tests/long_lane.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- This task is a real baseline E2E proof, not a new image-packaging task. Story 02 already proves the single-binary image contract, so the story-10 test may run `runner` and `source-bootstrap` on the host while using Docker for the real CockroachDB, PostgreSQL, and MOLT processes.
- The baseline scenario should seed the source before CDC setup, execute the explicit Cockroach bootstrap commands once, and then perform no further raw source-side commands after CDC setup completes.
- The test must stay fully real on the data path:
  - real CockroachDB server
  - real PostgreSQL server
  - real HTTPS webhook traffic
  - real helper-shadow persistence
  - real reconcile loop into destination tables
  - real MOLT verify against destination tables
- The host environment does not provide `cockroach` or `molt`, but it does provide Docker. The test harness should therefore create wrapper executables that proxy those public command names into Dockerized tooling rather than weakening the product interfaces.
- HTTPS trust must be explicit in the source bootstrap contract. The current `source-bootstrap` config only carries `webhook.base_url`, which is not enough for a default Dockerized CockroachDB to trust the local test certificate. Because this is greenfield and no backward compatibility is allowed, it is acceptable to extend the config shape here.
- If the first RED slices prove that HTTPS trust cannot be expressed cleanly through the `source-bootstrap` render contract, or that a host-run `runner verify` cannot drive real MOLT through a simple wrapper executable, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Extend the `source-bootstrap` config contract with one explicit HTTPS trust input:
  - `webhook.ca_cert_path: <path>`
- Keep the `source-bootstrap` public CLI unchanged:
  - `source-bootstrap render-bootstrap-script --config <path>`
- Keep the runtime public CLI unchanged:
  - `runner run --config <path>`
  - `runner verify --config <path> --mapping <id> --source-url <cockroach-url> [--allow-tls-mode-disable]`
- Encode the HTTPS trust material inside the rendered Cockroach webhook sink URI, not in the long-lane test and not in ad hoc shell glue.
- Treat `crates/runner/tests/long_lane.rs` as the primary boundary-cleanup target:
  - move Docker/process/config/wrapper orchestration into a shared `tests/support` module
  - keep ignored long tests focused on observable operator behavior instead of container-shell plumbing
- Keep source-command logging inside the test harness wrapper executable, not inside product code. The product should remain responsible only for rendering the bootstrap script and running the destination/runtime flows.

## Public Contract To Establish

- `source-bootstrap render-bootstrap-script` emits an operator-explicit shell script that:
  - enables Cockroach rangefeeds explicitly
  - captures `cluster_logical_timestamp()` explicitly
  - creates the changefeed explicitly
  - uses an HTTPS webhook sink URI that includes the configured CA material needed for Cockroach to trust the destination webhook certificate
  - prints bootstrap metadata including mapping id, selected tables, starting cursor, and job id
- One ignored long-lane E2E test proves that a default CockroachDB plus default PostgreSQL setup can be bootstrapped end to end without hidden manual intervention:
  - source schema and seed data are created before CDC setup
  - `runner run` bootstraps the helper schema automatically
  - the rendered source-bootstrap script is the only source-side CDC setup action
  - after the script completes, the test performs no extra raw source commands
  - helper shadow tables receive the initial scan
  - the reconcile loop drives the real destination tables to parity
  - `runner verify` passes against the real destination tables through real MOLT output
- The long-lane test must assert the explicit-source-setup rule concretely by inspecting the wrapper log and proving the only source-side commands after seed/setup are the expected bootstrap commands.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this task:
  - `crates/runner/tests/long_lane.rs` currently mixes Docker lifecycle, TLS client setup, runtime process management, config materialization, and behavioral assertions in one file
- Required cleanup:
  - introduce a shared ignored-test support layer under `crates/runner/tests/support/`
  - keep long-test support responsible for Docker resources, wrapper executables, temp configs, and polling helpers
  - keep test files responsible for behavior assertions only
- Secondary cleanup:
  - keep CA-query rendering inside `source-bootstrap` config/render code, not duplicated in tests
  - keep wrapper-script generation in one support module so the long tests do not duplicate shell snippets for `cockroach` and `molt`
  - do not spread baseline-E2E assertions across multiple unrelated test files; keep the scenario readable as one operator journey

## Files And Structure To Add Or Change

- [x] `crates/source-bootstrap/src/config/mod.rs`
  - add typed access to the explicit CA certificate path for webhook bootstrap
- [x] `crates/source-bootstrap/src/config/parser.rs`
  - require and validate the new `webhook.ca_cert_path` field
- [x] `crates/source-bootstrap/src/render.rs`
  - render the CA-qualified HTTPS webhook sink URI from typed config instead of leaving TLS trust implicit
- [x] `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - extend the public render contract coverage for the explicit HTTPS trust material
- [x] `crates/source-bootstrap/tests/fixtures/valid-source-bootstrap-config.yml`
- [x] `crates/source-bootstrap/tests/fixtures/invalid-source-bootstrap-config.yml`
  - update fixtures for the new required config shape
- [x] `crates/runner/tests/support/mod.rs`
  - new shared support entrypoint for ignored E2E helpers
- [x] `crates/runner/tests/support/e2e_harness.rs`
  - own Docker lifecycle, temp config generation, wrapper scripts, polling, and database helpers for long-lane scenarios
- [x] `crates/runner/tests/long_lane.rs`
  - trim the existing file to behavior-focused ignored tests that consume the shared support layer
- [x] `crates/runner/tests/default_bootstrap_long_lane.rs`
  - new ignored baseline E2E contract for default CockroachDB to PostgreSQL bootstrap from scratch
- [x] `crates/runner/tests/support/runner_image_harness.rs`
  - keep the existing image long-lane contract on a separate support boundary so long-lane helpers stay warning-free under `clippy -D warnings`
- [x] No dedicated `crates/runner/tests/fixtures/` additions were needed
  - dynamic temp-file generation covered the TLS trust and operator-visible config inputs for the new long-lane scenario

## TDD Execution Order

### Slice 1: Explicit HTTPS Trust In The Rendered Bootstrap Script

- [x] RED: extend `crates/source-bootstrap/tests/bootstrap_contract.rs` with a failing contract that requires `render-bootstrap-script` to encode explicit HTTPS trust material from config into each rendered webhook sink URI
- [x] GREEN: add the typed `webhook.ca_cert_path` config field, validate it, load the certificate bytes, and render the CA-qualified sink URI
- [x] REFACTOR: keep CA loading/encoding and sink-URI rendering behind one typed webhook/bootstrap boundary rather than scattering string concatenation across config parsing and script rendering

### Slice 2: Tracer Bullet For The Real Baseline Operator Path

- [x] RED: add one ignored failing long-lane test that starts default CockroachDB and PostgreSQL containers, launches `runner run` on the host, renders the bootstrap script through `source-bootstrap`, executes it through a Docker-backed `cockroach` wrapper, and waits for the destination real tables to match the seeded source rows
- [x] GREEN: implement only the minimum shared harness support needed for that single scenario to converge end to end
- [x] REFACTOR: move Docker/resource/process orchestration out of `long_lane.rs` into `tests/support/e2e_harness.rs` so the scenario reads as behavior rather than shell plumbing

### Slice 3: Prove The Cockroach Changes Are Explicit And Finite

- [x] RED: extend the ignored baseline test to assert the `cockroach` wrapper log contains the explicit rangefeed setting, cursor capture, and changefeed creation calls, and that no additional raw source-side commands occur after bootstrap completes
- [x] GREEN: make the harness log and expose the wrapped `cockroach` invocations cleanly enough for the public scenario to assert that operator rule
- [x] REFACTOR: keep wrapper logging as a harness concern only; do not leak test-only logging hooks into product code

### Slice 4: Prove Automatic Helper Bootstrap And Shadow Persistence

- [x] RED: extend the ignored baseline test to assert that `runner run` automatically creates `_cockroach_migration_tool`, seeds stream/table tracking state, and lands the initial scan into helper shadow tables before the final parity assertion
- [x] GREEN: fix only the real gaps exposed by the end-to-end run, if any, and keep failures loud
- [x] REFACTOR: centralize helper-schema inspection queries in the harness so assertions stay readable and the SQL shape is not duplicated across ignored tests

### Slice 5: Real MOLT Verify Against Real Destination Tables

- [x] RED: extend the ignored baseline test so it runs `runner verify` against the live Cockroach/PostgreSQL pair using a Docker-backed `molt` wrapper executable, then assert the command succeeds and the emitted summary/artifacts refer to the real migrated tables only
- [x] GREEN: implement the smallest harness support needed to drive real MOLT through the existing verify interface
- [x] REFACTOR: keep MOLT wrapper construction in the shared harness and avoid introducing a second verify command path or test-only product config shape

### Slice 6: Full Repository Lanes And Final Boundary Review

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm the long-lane support code is centralized and the `source-bootstrap` HTTPS trust contract remains typed instead of stringly

## TDD Guardrails For Execution

- Every new behavior assertion must fail before production code changes are made. If a proposed assertion already passes, replace it with the next uncovered behavior.
- Prefer observable end-to-end assertions through the public CLIs over unit tests of private helpers.
- Do not weaken the scenario by faking webhook payloads, bypassing the real reconcile loop, or replacing MOLT with a fake binary in the final ignored baseline test.
- Do not hide missing tooling behind silent skips. If Dockerized wrappers fail to start or a real tool invocation fails, treat that as a real test failure.
- Do not add extra raw source commands after CDC setup just to make the baseline scenario pass. If the scenario needs that, the design is wrong and the plan must switch back to `TO BE VERIFIED`.
- Do not let CA handling become a test-only hack. The rendered bootstrap script must carry the explicit trust material required for the HTTPS webhook path.

## Boundary Review Checklist

- [x] No CA-query assembly lives only in tests; it is owned by the `source-bootstrap` render boundary
- [x] No ignored long test owns Docker lifecycle, wrapper script content, and assertions in the same file
- [x] No fake webhook injection bypasses the real HTTPS endpoint
- [x] No post-bootstrap raw Cockroach commands are used to advance the scenario
- [x] No helper-table names leak into MOLT verification filters
- [x] No error from Dockerized tool wrappers is swallowed or silently downgraded

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
