# Plan: Build The HTTPS Webhook Server And Table-Routing Runtime

## References

- Task: `.ralph/tasks/story-06-destination-ingest/01-task-build-https-webhook-server-and-routing.md`
- Previous task plan: `.ralph/tasks/story-05-schema-validation/02-task-generate-helper-shadow-ddl-and-dependency-order_plans/2026-04-18-helper-shadow-ddl-plan.md`
- Design: `designs/crdb-to-postgres-cdc/04_operational_model.md`
- Design: `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- Test strategy: `designs/crdb-to-postgres-cdc/07_test_strategy.md`
- Investigation: `investigations/cockroach-webhook-cdc/README.md`
- Current implementation: `crates/runner/src/lib.rs`
- Current implementation: `crates/runner/src/postgres_bootstrap.rs`
- Current implementation: `crates/source-bootstrap/src/render.rs`
- Current tests: `crates/runner/tests/bootstrap_contract.rs`
- Current tests: `crates/runner/tests/cli_contract.rs`
- Current tests: `crates/runner/tests/long_lane.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumption

- The task markdown plus the design and investigation docs above are treated as approval for the ingress boundary, the HTTPS requirement, and the multi-mapping runtime shape.
- The current single shared source-bootstrap webhook URL is not sufficient for multi-mapping resolved-message routing because observed resolved payloads are shaped as `{"resolved":"..."}` and do not carry source database metadata.
- This task should therefore establish one mapping-scoped HTTPS ingest path per configured mapping, and source-bootstrap must render those paths instead of pointing every changefeed at one shared `/events` URL.
- Task 01 must stay honest about acknowledgement semantics:
  - it may parse and route row batches and resolved messages
  - it must not return `200` for a routed message until tasks 02 and 03 implement the corresponding durable PostgreSQL persistence
- If the first execution slices prove that mapping-scoped paths are the wrong public boundary, or that task 01 cannot expose a real TLS server without also changing the CLI/runtime boundary more deeply than planned here, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Interface And Boundary Decisions

- Replace the fake `runner run` startup summary contract with a real long-running runtime boundary:
  - `run` must bootstrap PostgreSQL helper structures first
  - then bind the HTTPS listener
  - then keep serving until shutdown or fatal error
- `lib.rs` should stop acting as a courier for long-running runtime internals. Keep CLI parsing thin and move ingress ownership into a dedicated runtime module, for example:
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/webhook_runtime/payload.rs`
  - `crates/runner/src/webhook_runtime/routing.rs`
- Keep TLS loading inside the webhook runtime module instead of creating a detached top-level TLS utility module. This is a direct `wrong-placeism` cleanup from `improve-code-boundaries`.
- Introduce one typed runtime plan derived once from validated config:
  - `RunnerWebhookPlan`
  - `MappingWebhookRoute`
  - `TableRoute`
- That runtime plan should reduce config into the facts actually needed at request time:
  - listener address
  - TLS cert/key paths
  - mapping id to destination database mapping
  - allowed source database name for each mapping
  - allowed source tables for each mapping
  - deterministic ingest path for each mapping
- The source-bootstrap side should stop treating the webhook URL as one fully rendered sink string. Replace that stringly contract with a mapping-aware base URL contract, for example:
  - rename `webhook.url` to `webhook.base_url`
  - render one sink URL per mapping as `<base_url>/ingest/<mapping_id>`
- The webhook payload boundary should be typed and explicit:
  - `WebhookRequest::RowBatch(RowBatchRequest)`
  - `WebhookRequest::Resolved(ResolvedRequest)`
  - `RowEvent`
  - `SourceMetadata`
- Routing must be two-stage and loud:
  - first by path mapping id
  - then, for row batches, by `source.database_name` and `source.table_name`
- Row batches for a mapping must reject:
  - unknown mapping id
  - missing `source`
  - source database mismatch
  - source table not listed in that mapping
  - mixed-table payloads that span multiple selected tables
- Resolved messages route by mapping id path alone because the body shape does not identify the source database.
- Task 01 should establish a narrow dispatch boundary for later persistence tasks, for example:
  - `DispatchTarget::RowBatch { mapping_id, table, rows }`
  - `DispatchTarget::Resolved { mapping_id, resolved }`
- Until tasks 02 and 03 land, the HTTP handler must fail loudly after parse-and-route using a typed `RunnerIngressError::PersistenceNotImplemented` path rather than pretending success.

## Public Contract To Establish

- `runner run --config <path>` becomes a long-running command that:
  - bootstraps PostgreSQL helper structures exactly as before
  - then exposes a real TLS HTTP server from the same binary and process
- The server exposes:
  - `GET /healthz`
  - `POST /ingest/:mapping_id`
- `GET /healthz` returns `200` over HTTPS after bootstrap and listener startup are complete.
- `POST /ingest/:mapping_id` accepts only the two observed body shapes:
  - row batch: `{"payload":[...],"length":N}`
  - resolved watermark: `{"resolved":"..."}`
- Unknown mapping ids return `404`.
- Invalid JSON or unsupported body shapes return `400`.
- Validly parsed and correctly routed requests in task 01 must still return a loud non-`200` failure until durable persistence is implemented in tasks 02 and 03.
- Source-bootstrap render output must now create one changefeed sink path per mapping instead of one shared sink URL for all mappings.
- The Docker and README quick-start examples must show the new mapping-scoped ingest path contract so novice users can start the runtime without reverse engineering the route shape.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten in this story:
  - the current `run` path mixes CLI command rendering, PostgreSQL bootstrap orchestration, and the future long-running ingress runtime into one fake summary-returning branch
- Required cleanup from that:
  - remove the assumption that every command produces a finished `Display` summary immediately
  - keep validated config as the only source of webhook bind/TLS facts
  - derive route plans once and pass typed runtime data downward instead of re-looking-up mappings inside handlers
  - keep source-bootstrap sink-path rendering and runner route matching derived from the same mapping-id rule so those two crates cannot drift
- Secondary cleanup:
  - keep payload parsing types in the webhook module, not in `lib.rs` or bootstrap
  - avoid route-string `format!` soup at call sites by giving `MappingWebhookRoute` a `Display` implementation or equivalent single render path
  - keep new fields and helpers private unless another module truly needs them

## Files And Structure To Add Or Change

- [x] `Cargo.toml`
  - add the minimal shared dependencies needed for real HTTPS server support and TLS test clients
- [x] `crates/runner/Cargo.toml`
  - add runtime and test dependencies for the HTTPS server plus TLS client contract tests
- [x] `crates/runner/src/lib.rs`
  - keep CLI dispatch thin and stop forcing `run` through the short-lived command-output boundary
- [x] `crates/runner/src/error.rs`
  - add typed ingress, payload-parse, routing, TLS-load, and unimplemented-persistence errors
- [x] `crates/runner/src/webhook_runtime/mod.rs`
  - own HTTPS startup, route wiring, and request dispatch
- [x] `crates/runner/src/webhook_runtime/payload.rs`
  - typed request parsing for row batches and resolved messages
- [x] `crates/runner/src/webhook_runtime/routing.rs`
  - typed mapping/table routing plan derived from validated config
- [x] `crates/runner/src/config/mod.rs`
  - expose only the reduced facts the runtime needs; do not duplicate validated webhook config elsewhere
- [x] `crates/runner/tests/cli_contract.rs`
  - assert help still exposes `run`
- [x] `crates/runner/tests/webhook_contract.rs`
  - new HTTPS contract tests for health, path routing, and payload-shape dispatch
- [x] `crates/runner/tests/long_lane.rs`
  - update the container/runtime lane so `runner run` is verified as a long-running HTTPS process instead of an immediate summary command
- [x] `crates/source-bootstrap/src/config/mod.rs`
  - rename the webhook base URL field if needed for the new mapping-scoped route contract
- [x] `crates/source-bootstrap/src/config/parser.rs`
  - validate the new webhook base URL shape once inside config parsing
- [x] `crates/source-bootstrap/src/render.rs`
  - emit one mapping-scoped ingest URL per changefeed
- [x] `crates/source-bootstrap/tests/bootstrap_contract.rs`
  - lock in the new rendered sink shape
- [x] `README.md`
  - document the mapping-scoped ingest URLs and the long-running HTTPS runtime

## TDD Execution Order

### Slice 1: Tracer Bullet For Real TLS Startup

- [x] RED: add one failing HTTPS contract test that starts `runner run --config <fixture>` and proves a TLS client can successfully call `GET /healthz`
- [x] GREEN: introduce the minimal runtime/server boundary so bootstrap still runs, the process stays alive, and the health endpoint answers over real TLS
- [x] REFACTOR: remove any fake immediate-startup summary assumption left in `lib.rs`

### Slice 2: Mapping-Scoped Route Contract

- [x] RED: add failing source-bootstrap and runner contract coverage proving that each mapping gets its own `/ingest/<mapping_id>` path and that unknown mapping ids return `404`
- [x] GREEN: derive mapping-scoped route paths from mapping ids and render the same rule from source-bootstrap
- [x] REFACTOR: centralize route rendering so bootstrap-script output and server routing cannot drift

### Slice 3: Payload-Shape Parsing

- [x] RED: add failing HTTPS request tests for the two observed body shapes:
  - valid row batch
  - valid resolved watermark
  - malformed or unsupported JSON
- [x] GREEN: parse into typed `WebhookRequest` values and reject bad shapes with `400`
- [x] REFACTOR: keep parse logic in the webhook module, not in handlers or `lib.rs`

### Slice 4: Table Routing For Row Batches

- [x] RED: add failing contract tests proving that row batches route only when `source.database_name` and `source.table_name` match the selected mapping tables, and fail loudly for mismatches
- [x] GREEN: implement typed routing from path mapping id plus row `source` metadata into `DispatchTarget::RowBatch`
- [x] REFACTOR: reduce config into one reusable `RunnerWebhookPlan` so handlers do not repeatedly search raw mappings

### Slice 5: Resolved Routing Without Shared `/events`

- [x] RED: add failing coverage proving that resolved requests route by mapping path and no longer depend on missing body metadata
- [x] GREEN: complete the mapping-path contract and route resolved requests into `DispatchTarget::Resolved`
- [x] REFACTOR: remove any leftover assumption that a single shared webhook URL can safely support all mappings

### Slice 6: Honest Pre-Persistence Failure Boundary

- [x] RED: add failing contract tests proving that a validly parsed and routed request does not return `200` yet
- [x] GREEN: surface a typed `PersistenceNotImplemented` failure path after dispatch so the runtime remains honest until tasks 02 and 03 land
- [x] REFACTOR: keep this failure localized behind one dispatch boundary so later persistence work swaps implementations instead of rewriting server wiring

### Slice 7: Repository Lanes

- [x] RED: run `make check`, `make lint`, `make test`, and `make test-long`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to remove any duplicate route shapes, fake startup summaries, or config/runtime drift

## Boundary Review Checklist

- [x] No shared `/events` contract remains for multi-mapping resolved routing
- [x] No webhook route strings are assembled ad hoc in both runner and source-bootstrap
- [x] No long-running `run` behavior is forced through the old short-lived `CommandOutput` assumption
- [x] No config validation for webhook paths, TLS files, or mapping facts lives outside config parsing
- [x] No handler re-searches raw config mappings after the typed runtime plan is built
- [x] No payload-shape mismatch or routing mismatch is swallowed or downgraded into fake success
- [x] TLS loading lives with the webhook runtime instead of a detached shared helper module

## Final Verification For The Execution Turn

- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
- [x] One final `improve-code-boundaries` pass after all lanes are green
- [x] Update the task file acceptance checkboxes and set `<passes>true</passes>` only after every lane passes

NOW EXECUTE
