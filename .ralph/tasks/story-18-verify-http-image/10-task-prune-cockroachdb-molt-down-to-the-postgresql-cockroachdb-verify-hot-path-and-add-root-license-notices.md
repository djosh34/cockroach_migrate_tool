## Task: Prune `cockroachdb_molt` down to the PostgreSQL/CockroachDB verify hot path and add root license notices <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Audit the `cockroachdb_molt` subrepo and remove every Go package path, command, fixture, telemetry hook, dependency, and document that does not directly contribute to the supported verification hot path for CockroachDB and PostgreSQL. The higher order goal is to keep the verify source slice operationally minimal and legally explicit so the repo root ships only code that participates in supported verification and clearly documents the licensing split.

In scope:
- inspect all Go package paths, commands, and reachable build/runtime paths under `cockroachdb_molt`
- remove support for databases other than PostgreSQL and CockroachDB, including MySQL, Oracle, and any other non-supported connectors, adapters, fixtures, or docs
- remove telemetry, analytics, phone-home code, extra metrics/reporting paths, and any other instrumentation that is not directly required to execute verification
- remove any other code, tests, assets, workflows, and docs that are not in the direct hot path for verification
- prove with tests and static checks that the remaining `cockroachdb_molt` slice is the minimum required for supported verification behavior
- add repo-root licensing material such as `LICENSE`, `THIRD_PARTY_NOTICES`, or equivalent files that clearly state the Rust code in the repo root is `All Rights Reserved - Joshua Azimullah`
- add an explicit repo-root reference to the Apache-2.0 license that applies to the `cockroachdb_molt` verify-derived component, including where that exception applies

Out of scope:
- expanding support beyond CockroachDB and PostgreSQL
- preserving unused upstream compatibility layers, telemetry, or multi-database abstractions
- adding new verify runtime capabilities unrelated to pruning and licensing clarity

Decisions already made:
- only the CockroachDB/PostgreSQL verification path is supported
- anything not directly contributing to verification should be removed, not left dormant
- telemetry should not remain in the `cockroachdb_molt` subrepo
- this is a greenfield repo, so dead upstream compatibility code should be deleted rather than preserved
- the repo root must clearly describe the licensing split between the proprietary Rust code and the Apache-2.0 `cockroachdb_molt` verify-derived code

</description>


<acceptance_criteria>
- [x] Red/green TDD proves every retained `cockroachdb_molt` Go package path is required by the PostgreSQL/CockroachDB verify hot path
- [x] All non-supported database paths, including MySQL, Oracle, and any other non-PostgreSQL/CockroachDB connectors or helpers, are removed from `cockroachdb_molt`
- [x] Telemetry and any other non-hot-path instrumentation are removed from `cockroachdb_molt`
- [x] Automated checks fail if removed database backends, telemetry paths, or other dead code re-enter the verify slice
- [x] Repo-root licensing files clearly state `All Rights Reserved - Joshua Azimullah` for the Rust code in the repo root and explicitly reference the Apache-2.0 license for the `cockroachdb_molt` verify-derived component
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-18-verify-http-image/10-task-prune-cockroachdb-molt-down-to-the-postgresql-cockroachdb-verify-hot-path-and-add-root-license-notices_plans/2026-04-19-pg-crdb-verify-hot-path-prune-and-root-licensing-plan.md</plan>
