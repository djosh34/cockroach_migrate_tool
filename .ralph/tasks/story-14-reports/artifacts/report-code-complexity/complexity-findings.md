# Complexity Findings

This artifact records the concrete hotspots and classifies whether their complexity appears justified, accidental, or still uncertain.

## Finding 1: `schema_compare/mod.rs` Is The Largest Real Complexity Center

- Evidence: [crates/runner/src/schema_compare/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/schema_compare/mod.rs:1) is 801 lines and still owns statement dispatch, table creation parsing, alter parsing, index parsing, type normalization, CSV splitting, parenthesis matching, and semantic comparison.
- Why reasoning cost increases: the subsystem root blends parsing mechanics with domain comparison policy, so changes to one concern require reloading the whole file mentally.
- Why some complexity is justified: semantic schema comparison across two exported formats is genuinely domain-heavy, and the code does real work instead of wrapping library calls for no reason.
- Classification: partly justified, partly accidental. The domain is real, but the file boundary has not kept pace with the amount of logic now living there.

## Finding 2: `postgres_bootstrap.rs` Mixes Bootstrap DDL, Catalog Introspection, And Helper-Plan Derivation

- Evidence: [crates/runner/src/postgres_bootstrap.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/postgres_bootstrap.rs:1) owns destination-group connection handling, helper schema creation, catalog reads for columns/primary keys/foreign keys, helper-table DDL execution, and tracking-state seeding.
- Why reasoning cost increases: one module now handles both operational side effects and schema-shape discovery. That means understanding bootstrap behavior requires switching constantly between connection lifecycle, SQL catalog queries, domain reconstruction, and helper setup.
- Why some complexity is justified: the runtime genuinely needs to learn destination schema and create helper objects before steady-state operation.
- Classification: accidental boundary blur. The sequence is necessary, but the current module boundary is too broad.

## Finding 3: `helper_plan.rs` Combines Deep Domain Logic With Artifact Rendering

- Evidence: [crates/runner/src/helper_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/helper_plan.rs:1) contains `MappingHelperPlan`, `HelperShadowTablePlan`, `ReconcileOrder::from_schema`, plus `MappingReadme`, `HelperTablesSql`, `ReconcileOrderText`, and filesystem writes.
- Why reasoning cost increases: deriving helper tables and topologically sorting foreign-key dependencies are domain operations; rendering `README.md` and `*.sql` text is output formatting. Keeping both in one file makes the module harder to scan and harder to extend cleanly.
- Why some complexity is justified: the reconcile-order topological sort in [helper_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/helper_plan.rs:260) is a good example of real complexity that belongs somewhere.
- Classification: accidental. The domain plan is legitimate; the rendering concerns should not live in the same boundary.

## Finding 4: `runtime_plan.rs` Repeats Mapping Identity Across Several Shapes

- Evidence: [crates/runner/src/runtime_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/runtime_plan.rs:1) introduces `ConfiguredMappingPlan`, `DestinationGroupPlan`, `RunnerRuntimePlan`, and `MappingRuntimePlan`, while [crates/runner/src/postgres_bootstrap.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/postgres_bootstrap.rs:465) adds `MappingBootstrapPlan`.
- Why reasoning cost increases: a reader must keep straight which mapping shape is raw config, grouped startup plan, bootstrap request, helper plan, or runtime plan. Some of that progression is useful, but some of it only repackages the same identity, connection, and selected-table data.
- Why some complexity is justified: `MappingRuntimePlan` genuinely deepens the mapping with helper-table lookup and ordered reconcile metadata.
- Classification: mixed. The pipeline is valid, but at least one translation layer looks thinner than it should be.

## Finding 5: `molt_verify/mod.rs` Is An Adapter That Has Started Acting Like A Subsystem

- Evidence: [crates/runner/src/molt_verify/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/molt_verify/mod.rs:1) builds CLI arguments, spawns the process, parses JSON lines, computes mismatch verdicts, writes raw logs, writes summary artifacts, and formats user-visible output.
- Why reasoning cost increases: external command execution, log parsing, artifact persistence, and result-policy decisions are separate concerns that all live in one file.
- Why some complexity is justified: `verify` is genuinely more than a thin shell wrapper because it constrains `molt` to mapping-selected tables and produces its own report artifacts.
- Classification: mildly accidental. The behavior is correct, but the adapter is taking on too many jobs.

## Finding 6: The Shared Error Layer Is Useful But Oversized

- Evidence: [crates/runner/src/error.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/error.rs:1) is 522 lines and covers nearly every subsystem in the crate.
- Why reasoning cost increases: a central error catalog improves consistency, but once it grows this large it becomes an indirect index of how many subsystems the crate actually contains.
- Why some complexity is justified: this crate does need explicit, user-facing failures across config, schema compare, bootstrap, runtime, verify, and cutover paths.
- Classification: acceptable but warning-level. It is not ornamental, but it confirms `runner` is operating as a very broad crate.

## Finding 7: The Webhook Runtime Is A Positive Example Of Good Boundary Splitting

- Evidence: [crates/runner/src/webhook_runtime/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/mod.rs:1), [payload.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/payload.rs:1), [routing.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/routing.rs:1), and [persistence.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/persistence.rs:1) divide HTTP, parsing, routing, and persistence cleanly.
- Why reasoning cost decreases: each child module owns one real responsibility, and the interactions are simple to describe.
- Classification: justified and healthy complexity. This subsystem demonstrates the repo’s best current internal boundary discipline.

## Finding 8: `validated_schema` And `sql_name` Are Real Deep Modules, Not Ornament

- Evidence:
  - [crates/runner/src/validated_schema.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/validated_schema.rs:1)
  - [crates/runner/src/sql_name.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/sql_name.rs:1)
- Why reasoning cost decreases: they absorb SQL identifier normalization and semantic schema shapes so higher layers can work in domain terms instead of raw strings.
- Classification: justified and KISS-aligned. These abstractions remove complexity instead of adding ceremony.

## Finding 9: Production Code Still Contains Internal `panic!` And `expect()` Boundaries

- Evidence:
  - [crates/runner/src/runtime_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/runtime_plan.rs:132)
  - [crates/runner/src/postgres_bootstrap.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/postgres_bootstrap.rs:45)
  - [crates/runner/src/sql_name.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/sql_name.rs:40)
  - [crates/runner/src/schema_compare/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/schema_compare/mod.rs:770)
  - [crates/runner/src/molt_verify/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/molt_verify/mod.rs:238)
- Why reasoning cost increases: internal assumptions are encoded as process-aborting invariants instead of normal error paths. That makes the code less uniform and harder to reason about under unexpected input or future refactor mistakes.
- Classification: unnecessary risk. These are not the dominant complexity problem, but they are a KISS violation because they create hidden exceptional control flow.

## Finding 10: The Test Surface Is Public-Interface Focused But Structurally Heavy

- Evidence:
  - top-level integration files are large, such as [bootstrap_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/bootstrap_contract.rs:1), [reconcile_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/reconcile_contract.rs:1), and [webhook_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/webhook_contract.rs:1)
  - the support harness is also large, especially [e2e_harness.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/support/e2e_harness.rs:1), [multi_mapping_harness.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/support/multi_mapping_harness.rs:1), and [webhook_chaos_gateway.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/support/webhook_chaos_gateway.rs:1)
  - helper functions and local Postgres bootstrapping logic are duplicated across several test files rather than fully centralized
- Why reasoning cost increases: the integration suite behaves like a second orchestration layer. It is valuable, but it is also expensive to navigate.
- Why some complexity is justified: this project depends on real runtime behavior, containerized environments, and public-CLI contracts, so the suite cannot be trivially small.
- Classification: mixed. The testing style is good, but the harness boundary can be flattened.

## Overall Complexity Judgment

The codebase is not drifting into abstraction-for-abstraction’s-sake. Most complexity comes from real migration concerns:

- semantic schema compatibility
- helper shadow-table planning
- PostgreSQL bootstrap and tracking
- webhook ingest and reconcile execution
- external verification and cutover safety

The problem is narrower and more fixable than “the design is overengineered.” The real issue is that several modules are shouldering both deep domain logic and adjacent translation or rendering work. The code is still understandable, but the current boundary discipline inside `runner` is weaker than the outer crate-level design.
