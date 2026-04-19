# Module Inventory

## Workspace Shape

The production workspace contains 31 Rust source files under `crates/*/src` for a total of 6,819 source lines. The distribution is uneven:

- `ingest-contract`: 1 source file, 25 lines
- `source-bootstrap`: 5 source files, 397 lines
- `runner`: 25 source files, 6,397 lines

This is materially a single-complex-crate workspace with two thin companion crates.

## Crate Inventory

### `ingest-contract`

#### `lib.rs`

- Path: [crates/ingest-contract/src/lib.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/ingest-contract/src/lib.rs:1)
- Responsibility: centralize ingest path and URL rendering.
- Major types/functions:
  - `MappingIngestPath`
  - `render_mapping_ingest_url`
- Boundary role: shared contract utility consumed by `source-bootstrap`.
- Assessment: clean and appropriately tiny. This is one of the clearest boundaries in the repo because it owns exactly one stable concept and does not leak unrelated policy.

### `source-bootstrap`

#### `lib.rs`

- Path: [crates/source-bootstrap/src/lib.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/source-bootstrap/src/lib.rs:1)
- Responsibility: CLI dispatch for the source-side bootstrap binary.
- Major types/functions:
  - `Cli`
  - `Command`
  - `execute`
- Boundary role: public binary surface.
- Assessment: clean. It delegates quickly and does not retain hidden runtime state.

#### `main.rs`

- Path: [crates/source-bootstrap/src/main.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/source-bootstrap/src/main.rs:1)
- Responsibility: process entrypoint and exit-code mapping.
- Boundary role: bootstrap shell.
- Assessment: minimal and honest.

#### `config/mod.rs`

- Path: [crates/source-bootstrap/src/config/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/source-bootstrap/src/config/mod.rs:1)
- Responsibility: typed bootstrap configuration model.
- Major types/functions:
  - `BootstrapConfig`
  - `WebhookConfig`
  - `SourceMapping`
  - `SourceSelection`
  - `TableName`
- Boundary role: data-shape layer between parsed YAML and render logic.
- Assessment: mostly clean. It exposes only the fields the renderer needs.

#### `config/parser.rs`

- Path: [crates/source-bootstrap/src/config/parser.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/source-bootstrap/src/config/parser.rs:1)
- Responsibility: YAML decoding, relative-path resolution, CA certificate loading, table-name validation, and raw-to-typed config transformation.
- Boundary role: parser and validator.
- Assessment: reasonable for a boundary parser, though it already combines filesystem concerns, validation rules, and URL-encoding support in one file.

#### `render.rs`

- Path: [crates/source-bootstrap/src/render.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/source-bootstrap/src/render.rs:1)
- Responsibility: render the operator-reviewed CockroachDB bootstrap shell script.
- Major types/functions:
  - `RenderedScript`
  - `render_mapping_block`
  - `shell_quote`
  - `sql_literal`
  - `render_job_variable_name`
- Boundary role: rendering layer.
- Assessment: cohesive. It does string-heavy work, but that is its actual job.

#### `error.rs`

- Path: [crates/source-bootstrap/src/error.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/source-bootstrap/src/error.rs:1)
- Responsibility: bootstrap error taxonomy.
- Boundary role: user-facing error translation.
- Assessment: compact and still proportional to the crate.

### `runner`

#### `lib.rs`

- Path: [crates/runner/src/lib.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/lib.rs:1)
- Responsibility: public CLI and top-level dispatch for all destination-side operations.
- Major types/functions:
  - `Cli`
  - `Command`
  - `execute`
  - `CommandOutput`
- Boundary role: public runtime interface.
- Assessment: clean outer shell, but it fans into many internal subsystems.

#### `main.rs`

- Path: [crates/runner/src/main.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/main.rs:1)
- Responsibility: runtime creation and process exit mapping.
- Boundary role: binary bootstrap.
- Assessment: intentionally thin.

#### `config/mod.rs`

- Path: [crates/runner/src/config/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/config/mod.rs:1)
- Responsibility: typed runner configuration model plus helper accessors.
- Major types/functions:
  - `RunnerConfig`
  - `LoadedRunnerConfig`
  - `MappingConfig`
  - `SourceConfig`
  - `DestinationConfig`
  - `PostgresConnectionConfig`
  - `WebhookConfig`
  - `TlsConfig`
  - `ReconcileConfig`
  - `VerifyConfig`
- Boundary role: config DTO layer.
- Assessment: useful boundary, but it introduces a second family of mapping/table/connection shapes that overlaps conceptually with runtime-plan and bootstrap request shapes.

#### `config/parser.rs`

- Path: [crates/runner/src/config/parser.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/config/parser.rs:1)
- Responsibility: YAML decoding and validation for runner config.
- Boundary role: parser and validator.
- Assessment: expected boundary code, though it is already a relatively large parser for a small project.

#### `error.rs`

- Path: [crates/runner/src/error.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/error.rs:1)
- Responsibility: central error taxonomy spanning config, artifacts, helper planning, bootstrap, runtime planning, webhook ingest, reconcile, schema compare, verify, and cutover readiness.
- Boundary role: cross-cutting error layer.
- Assessment: this is structurally convenient but also a warning sign. A single 522-line error file often means the crate is acting as many subsystems at once.

#### `runtime_plan.rs`

- Path: [crates/runner/src/runtime_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/runtime_plan.rs:1)
- Responsibility: transform validated config plus helper-plan output into startup and runtime execution plans.
- Major types/functions:
  - `RunnerStartupPlan`
  - `ConfiguredMappingPlan`
  - `DestinationDatabaseKey`
  - `DestinationGroupPlan`
  - `RunnerRuntimePlan`
  - `DestinationRuntimePlan`
  - `MappingRuntimePlan`
- Boundary role: orchestration/planning layer.
- Assessment: important deep module candidate, but likely overloaded because it both groups destination topology and enriches per-mapping runtime metadata.

#### `helper_plan.rs`

- Path: [crates/runner/src/helper_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/helper_plan.rs:1)
- Responsibility: derive helper shadow tables and reconcile order from validated schemas, then render operator-facing artifacts.
- Major types/functions:
  - `render_helper_plan`
  - `HelperPlanArtifacts`
  - `MappingHelperPlan`
  - `HelperShadowTablePlan`
  - `HelperColumnPlan`
  - `ReconcileOrder`
- Boundary role: mixed planning and artifact-rendering layer.
- Assessment: useful domain concentration, but the file likely mixes domain derivation with output formatting too tightly.

#### `postgres_setup.rs`

- Path: [crates/runner/src/postgres_setup.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/postgres_setup.rs:1)
- Responsibility: render PostgreSQL grants/setup artifacts.
- Boundary role: artifact renderer.
- Assessment: likely cohesive, pending deeper read.

#### `postgres_bootstrap.rs`

- Path: [crates/runner/src/postgres_bootstrap.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/postgres_bootstrap.rs:1)
- Responsibility: connect to PostgreSQL, create helper schema/tables/indexes, inspect destination catalog, and seed tracking state.
- Boundary role: bootstrap execution layer.
- Assessment: operationally central and likely a major complexity hotspot because it mixes connection lifecycle, DDL generation, catalog introspection, and helper-plan derivation.

#### `tracking_state.rs`

- Path: [crates/runner/src/tracking_state.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/tracking_state.rs:1)
- Responsibility: persist stream and reconcile progress metadata.
- Boundary role: persistence helper layer.
- Assessment: conceptually cohesive, though it spans webhook and reconcile update paths and may be becoming the shared state sink for multiple flows.

#### `validated_schema.rs`

- Path: [crates/runner/src/validated_schema.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/validated_schema.rs:1)
- Responsibility: semantic schema representation for selected tables, keys, constraints, and indexes.
- Boundary role: shared domain model.
- Assessment: this is a good central domain type layer. It looks like a legitimate deep module rather than ornamental abstraction.

#### `sql_name.rs`

- Path: [crates/runner/src/sql_name.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/sql_name.rs:1)
- Responsibility: normalize quoted SQL identifiers and qualified table names.
- Boundary role: naming utility with domain semantics.
- Assessment: another good small boundary. It removes stringly identifier handling from higher layers.

#### `schema_compare/mod.rs`

- Path: [crates/runner/src/schema_compare/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/schema_compare/mod.rs:1)
- Responsibility: parse exported CockroachDB/PostgreSQL schemas into semantic shapes and compare selected tables.
- Boundary role: schema-analysis subsystem root.
- Assessment: absolutely central to correctness, but 801 lines in one module suggests the subsystem has not yet been split along natural parsing/comparison seams.

#### `schema_compare/cockroach_export.rs`

- Path: [crates/runner/src/schema_compare/cockroach_export.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/schema_compare/cockroach_export.rs:1)
- Responsibility: read Cockroach schema export text.
- Boundary role: source-format adapter.
- Assessment: likely thin and appropriately scoped.

#### `schema_compare/postgres_export.rs`

- Path: [crates/runner/src/schema_compare/postgres_export.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/schema_compare/postgres_export.rs:1)
- Responsibility: read PostgreSQL schema export text.
- Boundary role: source-format adapter.
- Assessment: likely thin and appropriately scoped.

#### `schema_compare/report.rs`

- Path: [crates/runner/src/schema_compare/report.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/schema_compare/report.rs:1)
- Responsibility: render mismatch summaries and reporting shapes.
- Boundary role: reporting/output layer for the compare subsystem.
- Assessment: promising separation, though the subsystem root may still carry too much parsing detail.

#### `webhook_runtime/mod.rs`

- Path: [crates/runner/src/webhook_runtime/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/mod.rs:1)
- Responsibility: TLS setup, HTTP server startup, request handling, and dispatch.
- Boundary role: HTTP runtime root.
- Assessment: reasonably separated at first glance because parsing, routing, and persistence have child modules.

#### `webhook_runtime/payload.rs`

- Path: [crates/runner/src/webhook_runtime/payload.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/payload.rs:1)
- Responsibility: parse webhook JSON into typed requests.
- Boundary role: payload parser and request DTO layer.
- Assessment: cohesive and direct.

#### `webhook_runtime/routing.rs`

- Path: [crates/runner/src/webhook_runtime/routing.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/routing.rs:1)
- Responsibility: resolve parsed requests into persistence targets using runtime mapping metadata.
- Boundary role: routing layer.
- Assessment: clean boundary candidate. It transforms request shape into actionable internal targets without HTTP concerns.

#### `webhook_runtime/persistence.rs`

- Path: [crates/runner/src/webhook_runtime/persistence.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/persistence.rs:1)
- Responsibility: persist row mutations into helper tables.
- Boundary role: persistence execution layer.
- Assessment: cohesive, but still string-heavy because it renders mutation SQL inline.

#### `reconcile_runtime/mod.rs`

- Path: [crates/runner/src/reconcile_runtime/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/reconcile_runtime/mod.rs:1)
- Responsibility: spawn reconcile workers, run periodic passes, manage transactions, and track failures.
- Boundary role: execution orchestrator.
- Assessment: likely cohesive but orchestration-heavy.

#### `reconcile_runtime/upsert.rs`

- Path: [crates/runner/src/reconcile_runtime/upsert.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/reconcile_runtime/upsert.rs:1)
- Responsibility: apply one upsert reconcile step.
- Boundary role: table-level DML helper.
- Assessment: good split from the reconcile loop root.

#### `reconcile_runtime/delete.rs`

- Path: [crates/runner/src/reconcile_runtime/delete.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/reconcile_runtime/delete.rs:1)
- Responsibility: apply one delete reconcile step.
- Boundary role: table-level DML helper.
- Assessment: good split from the reconcile loop root.

#### `molt_verify/mod.rs`

- Path: [crates/runner/src/molt_verify/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/molt_verify/mod.rs:1)
- Responsibility: build and execute `molt verify`, parse JSON records, and write verification artifacts.
- Boundary role: external-tool adapter plus artifact writer.
- Assessment: functionally cohesive, but it combines command construction, record parsing, mismatch policy, and filesystem output.

#### `cutover_readiness/mod.rs`

- Path: [crates/runner/src/cutover_readiness/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/cutover_readiness/mod.rs:1)
- Responsibility: inspect stream/table tracking state and optionally invoke verify for final cutover readiness.
- Boundary role: readiness policy layer.
- Assessment: clear operator-facing purpose. It may overlap conceptually with tracking-state ownership and verify orchestration.

## First Boundary Read

The cleanest modules are the smallest, deepest ones:

- `ingest-contract`
- `sql_name`
- `validated_schema`
- `webhook_runtime/routing`
- `reconcile_runtime/upsert`
- `reconcile_runtime/delete`

The blurriest modules are the ones that mix domain rules with output or operational details:

- `helper_plan.rs`
- `postgres_bootstrap.rs`
- `molt_verify/mod.rs`
- `schema_compare/mod.rs`
- `error.rs`

Those files deserve the closest hotspot analysis in later slices.

## Test-Surface Inventory

The behavioral test suite is large and strongly integration-oriented. That is mostly a positive sign for correctness, but it is also part of the structural complexity picture.

Notable top-level test files:

- [crates/runner/tests/bootstrap_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/bootstrap_contract.rs:1): 969 lines
- [crates/runner/tests/cutover_readiness_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/cutover_readiness_contract.rs:1): 673 lines
- [crates/runner/tests/reconcile_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/reconcile_contract.rs:1): 1,528 lines
- [crates/runner/tests/webhook_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/webhook_contract.rs:1): 1,246 lines

Notable support files:

- [crates/runner/tests/support/e2e_harness.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/support/e2e_harness.rs:1): 1,549 lines
- [crates/runner/tests/support/multi_mapping_harness.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/support/multi_mapping_harness.rs:1): 797 lines
- [crates/runner/tests/support/webhook_chaos_gateway.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/support/webhook_chaos_gateway.rs:1): 609 lines
- [crates/runner/tests/support/default_bootstrap_harness.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/support/default_bootstrap_harness.rs:1): 435 lines

Inventory assessment:

- The tests mostly exercise public interfaces, which is good.
- The support side is large enough that it deserves the same boundary scrutiny as production modules.
- Repeated local helpers across multiple contract files indicate the test-support boundary is not yet as clean as the production CLI boundary.
