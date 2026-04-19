# Interaction Analysis

## Source Bootstrap Flow

`source-bootstrap` is the simplest end-to-end path in the repository:

- CLI entrypoint: [crates/source-bootstrap/src/lib.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/source-bootstrap/src/lib.rs:1)
- Config load and validation: [crates/source-bootstrap/src/config/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/source-bootstrap/src/config/mod.rs:1) and [parser.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/source-bootstrap/src/config/parser.rs:1)
- Script rendering: [crates/source-bootstrap/src/render.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/source-bootstrap/src/render.rs:1)

Actual call path:

1. `source_bootstrap::execute` parses the subcommand.
2. `BootstrapConfig::load` reads YAML from disk and delegates to the parser.
3. The parser validates text fields, table names, HTTPS webhook shape, and CA certificate readability.
4. `RenderedScript::from_config` turns the typed config into shell text.
5. `main.rs` prints the rendered script and exits.

Interaction assessment:

- This is a direct pipeline with minimal shape translation.
- The parser owns filesystem-sensitive validation and the renderer owns string-heavy shell output. That split is simple and stable.
- The only shared cross-crate interaction is `ingest-contract` for the ingest path contract, which is an appropriately tiny seam.

## Runner Command Flow

All destination-side operator commands enter through [crates/runner/src/lib.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/lib.rs:1). The outer CLI is compact, but the inner branches differ materially in complexity.

### `validate-config`

Call path:

1. `LoadedRunnerConfig::load` in [crates/runner/src/config/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/config/mod.rs:1)
2. YAML validation in [crates/runner/src/config/parser.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/config/parser.rs:1)
3. `ValidatedConfig::from` formatting in [crates/runner/src/lib.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/lib.rs:1)

Assessment:

- Simple and unsurprising.
- The config layer is honest about being a parser-plus-validator boundary.

### `render-postgres-setup`

Call path:

1. Load typed config.
2. `render_postgres_setup` in [crates/runner/src/postgres_setup.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/postgres_setup.rs:1)
3. `PostgresSetupPlan::from_config` derives grant plans from mapping config.
4. `write_to` renders `README.md` and per-mapping `grants.sql`.

Assessment:

- This is mostly a config-to-artifact flow with clean boundaries.
- It is direct enough that it does not feel overengineered.

### `compare-schema`

Call path:

1. Load typed config.
2. `compare_mapping_exports` in [crates/runner/src/schema_compare/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/schema_compare/mod.rs:1)
3. `validate_mapping_exports` selects only configured mapping tables.
4. `cockroach_export::parse_file` and `postgres_export::parse_file` read both schema exports.
5. `compare_selected_tables` compares normalized table, column, primary key, unique, foreign key, and index shapes.
6. `report.rs` renders mismatch text if semantic comparison fails.

Assessment:

- The behavior is conceptually strong and KISS-friendly at the command-contract level: semantic compare instead of raw diff.
- Internally, the subsystem root has become crowded. Parsing, normalization, and comparison all still live heavily inside `schema_compare/mod.rs`.

### `render-helper-plan`

Call path:

1. Load typed config.
2. `render_helper_plan` in [crates/runner/src/helper_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/helper_plan.rs:1)
3. Reuse `validate_mapping_exports` from the schema-compare subsystem.
4. Build `MappingHelperPlan` from `ValidatedSchema`.
5. Compute `ReconcileOrder` by topologically sorting selected tables through foreign-key edges.
6. Render helper DDL and reconcile-order artifacts.

Assessment:

- This is a good example of justified domain reuse: helper planning correctly depends on semantic schema validation.
- The module boundary is still blurry because the same file both derives the plan and renders artifact text.

### `verify`

Call path:

1. Load typed config.
2. `run_verify` in [crates/runner/src/molt_verify/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/molt_verify/mod.rs:1)
3. Build one `MoltVerifyRequest` from mapping config.
4. Derive schema and table filters from the configured mapping.
5. Spawn the external `molt verify` process.
6. Parse emitted JSON records.
7. Write raw and summarized artifacts under the configured report directory.

Assessment:

- The public behavior is simple and operator-oriented.
- The internal adapter is less simple than it first appears because it combines request construction, process execution, log parsing, mismatch policy, and artifact emission in one module.

### `cutover-readiness`

Call path:

1. Load typed config.
2. `run_cutover_readiness` in [crates/runner/src/cutover_readiness/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/cutover_readiness/mod.rs:1)
3. Build `CutoverReadinessRequest` from mapping config.
4. Read stream and table tracking rows from PostgreSQL.
5. Compute watermark alignment and per-table drain status.
6. If drained and aligned, call `run_verify`.

Assessment:

- This flow is easy to explain to an operator and matches the README runbook.
- The implementation is clean enough, though it overlaps conceptually with the tracking-state and verify subsystems rather than exposing a separate deeper boundary.

## `run` Runtime Flow

The `run` branch is the most important interaction path and the main reason `runner` dominates the complexity profile.

Actual control flow:

1. Load typed config in [crates/runner/src/config/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/config/mod.rs:1).
2. Convert config to `RunnerStartupPlan` in [crates/runner/src/runtime_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/runtime_plan.rs:1).
3. Group mappings by destination database and verify destination contract consistency.
4. Run `bootstrap_postgres` in [crates/runner/src/postgres_bootstrap.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/postgres_bootstrap.rs:1).
5. For each destination group, connect once, ensure helper schema exists, introspect destination catalog, derive helper plans, create helper tables and indexes, then seed tracking state.
6. Convert startup data plus helper plans into `RunnerRuntimePlan` in [runtime_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/runtime_plan.rs:1).
7. Launch `serve_webhook_runtime` and `serve_reconcile_runtime` concurrently via `tokio::try_join!`.

Data-flow interpretation:

- Config is first shaped into a planning form.
- Bootstrap enriches that planning form with observed destination-schema information.
- Runtime planning reshapes the result again for steady-state ingest and reconcile flows.

This is a meaningful domain pipeline, but it also introduces repeated mapping-centric shapes:

- `MappingConfig`
- `ConfiguredMappingPlan`
- `MappingBootstrapPlan`
- `MappingHelperPlan`
- `MappingRuntimePlan`

That is not automatically wrong. The important question is whether each step deepens the abstraction or just translates fields. In the current implementation, some of these shapes clearly deepen the domain:

- `MappingHelperPlan` adds helper-table and reconcile-order knowledge.
- `MappingRuntimePlan` adds helper-table lookup and ordered reconcile metadata.

Some are thinner and more suspicious:

- `MappingBootstrapPlan` mostly repackages `ConfiguredMappingPlan` to fit bootstrap internals.
- `ConfiguredMappingPlan` and `MappingRuntimePlan` share a lot of identity and selection data.

## Webhook Ingest Flow

Webhook serving is relatively well-layered:

- HTTP/TLS startup: [crates/runner/src/webhook_runtime/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/mod.rs:1)
- JSON parsing: [payload.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/payload.rs:1)
- request-to-target routing: [routing.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/routing.rs:1)
- helper-table mutation persistence: [persistence.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/persistence.rs:1)
- watermark state persistence: [crates/runner/src/tracking_state.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/tracking_state.rs:1)

Actual call path:

1. TLS server accepts a request.
2. `handle_ingest` resolves the mapping from runtime state.
3. `parse_webhook_request` converts JSON into either `RowBatchRequest` or `ResolvedRequest`.
4. `route_request` verifies source database and selected-table ownership, then returns a `DispatchTarget`.
5. `dispatch` calls either `persist_row_batch` or `persist_resolved_watermark`.

Assessment:

- This is one of the cleaner subsystems in the repo.
- The module split follows real responsibilities and keeps HTTP concerns out of persistence logic.
- The remaining complexity is mostly unavoidable SQL rendering and transaction handling.

## Reconcile Flow

Reconcile runtime is structurally simple but execution-heavy:

- orchestrator: [crates/runner/src/reconcile_runtime/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/reconcile_runtime/mod.rs:1)
- upsert SQL builder: [upsert.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/reconcile_runtime/upsert.rs:1)
- delete SQL builder: [delete.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/reconcile_runtime/delete.rs:1)
- tracking writes: [crates/runner/src/tracking_state.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/tracking_state.rs:1)

Actual call path:

1. Spawn one worker per mapping in each destination group.
2. Run a periodic loop.
3. Open a PostgreSQL transaction for the mapping.
4. Apply ordered upserts for helper-backed tables.
5. Apply ordered deletes in reverse dependency order.
6. Commit and persist tracking success, or roll back and persist failure.

Assessment:

- The high-level control flow is straightforward and easy to explain.
- Splitting `upsert.rs` and `delete.rs` out of the orchestrator is a good KISS move.
- The runtime still depends heavily on string-rendered SQL, but the rendering is localized enough to remain understandable.

## Test-Surface Interaction Read

The tests mostly target public behavior, which aligns well with the `tdd` skill’s preference for public-interface verification:

- CLI contracts
- README contracts
- schema-compare contracts
- bootstrap and webhook contracts
- end-to-end integrity and long-lane tests

That is a strength. The problem is structural duplication, not testing intent.

Evidence from the test tree:

- top-level test files are large, especially [reconcile_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/reconcile_contract.rs:1), [webhook_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/webhook_contract.rs:1), and [bootstrap_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/bootstrap_contract.rs:1)
- support harness files are also large, especially [e2e_harness.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/support/e2e_harness.rs:1), [multi_mapping_harness.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/support/multi_mapping_harness.rs:1), and [webhook_chaos_gateway.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/support/webhook_chaos_gateway.rs:1)
- helper utilities like `TestPostgres`, `pick_unused_port`, and fixture-path functions are repeated across multiple top-level integration tests instead of being fully centralized

Assessment:

- The test suite reflects the public contracts well.
- The support architecture is large enough to become its own subsystem, which increases reasoning cost and makes some production/test boundary duplication harder to avoid.
