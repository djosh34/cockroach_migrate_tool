# KISS Recommendations

This artifact focuses on simplification moves that remove shapes, files, or translation layers instead of adding wrappers.

## Recommendation 1: Keep The Existing Crate Layout

- Evidence: the workspace already has only one heavy crate, one thin bootstrap CLI, and one tiny shared contract crate.
- Recommendation: do not split `runner` into several shallow crates yet.
- Reasoning: the top-level structure is not the current problem. Moving complexity into more crates right now would likely create coordination overhead rather than deeper modules.

## Recommendation 2: Split `schema_compare/mod.rs` By Responsibility, Not By Pattern

- Evidence: [crates/runner/src/schema_compare/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/schema_compare/mod.rs:1) currently owns statement dispatch, parsing helpers, type normalization, and comparison logic.
- Recommendation: keep one `schema_compare` subsystem, but move parsing mechanics and normalization helpers out of `mod.rs` into deeper child modules.
- Good target split:
  - one parser-oriented module for statement parsing and low-level token helpers
  - one comparator-oriented module for semantic table comparison
  - keep [report.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/schema_compare/report.rs:1) as the mismatch rendering boundary
- Reasoning: this reduces one oversized file without inventing new conceptual layers.

## Recommendation 3: Separate Helper-Plan Derivation From Artifact Rendering

- Evidence: [crates/runner/src/helper_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/helper_plan.rs:1) currently mixes `MappingHelperPlan` and `ReconcileOrder` derivation with `README.md`, SQL, and text-file rendering.
- Recommendation: keep `MappingHelperPlan` and `ReconcileOrder` together, but move output renderers into a dedicated artifact module or file.
- Reasoning: deriving helper tables and topological reconcile order is deep domain logic. Rendering files is not. Splitting those concerns would make the plan boundary easier to reason about and easier to reuse.

## Recommendation 4: Flatten One Thin Mapping Translation Layer

- Evidence: the runtime pipeline uses `MappingConfig`, `ConfiguredMappingPlan`, `MappingBootstrapPlan`, `MappingHelperPlan`, and `MappingRuntimePlan`.
- Recommendation: remove or absorb the thinnest wrapper, most likely `MappingBootstrapPlan` in [crates/runner/src/postgres_bootstrap.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/postgres_bootstrap.rs:465), unless a future change gives it genuinely deeper behavior.
- Reasoning: `MappingHelperPlan` and `MappingRuntimePlan` add real domain value. `MappingBootstrapPlan` currently looks closer to a convenience projection than a stable abstraction.

## Recommendation 5: Extract PostgreSQL Catalog Reading Into One Deep Bootstrap Helper

- Evidence: [crates/runner/src/postgres_bootstrap.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/postgres_bootstrap.rs:1) contains both side-effectful bootstrap execution and low-level catalog loading for columns, primary keys, and foreign keys.
- Recommendation: introduce one internal deep helper focused only on “load selected destination schema from PostgreSQL,” and let `bootstrap_postgres` orchestrate around it.
- Reasoning: this would remove boundary blur without creating a new subsystem for its own sake.

## Recommendation 6: Keep The Webhook Subsystem As A Model For Other Boundaries

- Evidence: [crates/runner/src/webhook_runtime/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/mod.rs:1) plus [payload.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/payload.rs:1), [routing.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/routing.rs:1), and [persistence.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/webhook_runtime/persistence.rs:1) already follow real responsibility seams.
- Recommendation: preserve this structure and use it as the reference style when simplifying other subsystems.
- Reasoning: the best improvement here is not more change. It is resisting the urge to recombine cleanly separated layers.

## Recommendation 7: Replace Internal `panic!` And `expect()` Assumptions With Typed Failures

- Evidence:
  - [runtime_plan.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/runtime_plan.rs:132)
  - [postgres_bootstrap.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/postgres_bootstrap.rs:45)
  - [sql_name.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/sql_name.rs:40)
  - [schema_compare/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/schema_compare/mod.rs:770)
  - [molt_verify/mod.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/molt_verify/mod.rs:238)
- Recommendation: remove process-aborting invariants from normal production flows unless they are truly unreachable after type checking.
- Reasoning: this is a direct KISS improvement. Fewer hidden trapdoors means less mental branching when reading the code.

## Recommendation 8: Centralize Repeated Integration-Test Scaffolding

- Evidence: repeated `TestPostgres`, fixture-path, port-allocation, and command-runner helpers across [bootstrap_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/bootstrap_contract.rs:1), [cutover_readiness_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/cutover_readiness_contract.rs:1), [reconcile_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/reconcile_contract.rs:1), and [webhook_contract.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/tests/webhook_contract.rs:1).
- Recommendation: move the repeated local-Postgres and fixture helpers into one shared support boundary before adding more integration cases.
- Reasoning: this reduces support-code drift and keeps the test suite aligned with behavior-first testing instead of copy-pasted harness setup.

## Recommendation 9: Prefer Deep Domain Shapes Over More Output DTOs

- Evidence: [validated_schema.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/validated_schema.rs:1) and [sql_name.rs](/home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/crates/runner/src/sql_name.rs:1) are already doing useful work; the blurriness shows up when adjacent modules add extra projections mainly to support rendering or orchestration.
- Recommendation: when refactoring, remove needless projections before adding any new struct layer.
- Reasoning: this codebase already has the right instinct in its best modules. The simplest path forward is to deepen those modules, not to add more wrapper types.

## Final KISS Judgment

The implementation is still substantially KISS-oriented at the public-contract level:

- the CLI surface is small
- the workspace is compact
- abstractions are usually concrete and domain-tied

The drift is happening inside `runner`, where large modules increasingly combine domain logic with adjacent formatting, translation, or orchestration concerns. The next refactors should therefore be internal boundary cleanups, not architecture expansion.
