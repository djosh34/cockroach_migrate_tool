## Task: Remove the setup-sql Rust binary, its crate, tests, Dockerfiles, compose artifacts, CI/CD workflow entries, and all code/branch/doc references to it <status>completed</status> <passes>true</passes>

<description>
**Goal:** Completely delete the `setup-sql` Rust binary crate and every reference to it across the entire repository — workspace members, test harnesses, Dockerfiles, compose artifacts, CI/CD image catalog + publish + promote workflows, documentation across all doc-generator directories, and the README.

The higher-order goal is to replace the setup-sql Rust binary with a simpler, maintainer-friendly approach: human-readable docs in `./docs/setup_sql/` plus bash scripts that generate SQL from a YAML config. The first step is a clean, complete removal of the old binary so no stale artifacts remain.

This is a pure deletion/cleanup task — no TDD, no new tests. The existing test suite must still pass after removal.

In scope:
- Remove `crates/setup-sql` from root `Cargo.toml` workspace members
- Delete the entire `crates/setup-sql/` directory tree (Cargo.toml, Dockerfile, src/, tests/, fixtures/, support/)
- Delete `artifacts/compose/setup-sql.compose.yml`
- Remove the `setup-sql` entry from `.github/workflows/image-catalog.yml`
- Remove `setup_sql_image_ref` output and all setup-sql references from `.github/workflows/publish-images.yml`
- Remove or update `.github/workflows/promote-image-tags.yml` if it directly names setup-sql (it consumes the image catalog dynamically, verify)
- Remove the `COPY crates/setup-sql/Cargo.toml` line from root `Dockerfile`
- Delete or adapt all runner test code that references `setup_sql`, `source_bootstrap`, `setup-sql` compose artifacts, or `setup_sql_image_ref()` — this includes:
  - `crates/runner/tests/support/published_image_refs.rs` — `setup_sql_image_ref()` function
  - `crates/runner/tests/support/operator_cli_surface.rs` — `setup_sql()` method on `OperatorCliSurface`
  - `crates/runner/tests/support/novice_registry_only_harness.rs` — all setup_sql-related imports and methods
  - `crates/runner/tests/support/e2e_harness.rs` — all `source_setup_sql`, `destination_setup_sql`, `*bootstrap*` fields/methods
  - `crates/runner/tests/support/default_bootstrap_harness.rs` — `DEFAULT_SOURCE_SETUP_SQL`, `DEFAULT_DESTINATION_SETUP_SQL`
  - `crates/runner/tests/support/multi_mapping_harness.rs` — all `source_setup_sql`, `destination_setup_sql` references
  - `crates/runner/tests/support/composite_pk_exclusion_harness.rs` — `SOURCE_SETUP_SQL`, `DESTINATION_SETUP_SQL`
  - `crates/runner/tests/readme_operator_surface_contract.rs` — `readme_starts_with_setup_sql_before_runner_and_verify()` test
  - `crates/runner/tests/operator_cli_surface_contract.rs` — `setup_sql()` CLI surface assertions
  - `crates/runner/tests/novice_registry_only_contract.rs` — all setup_sql compose tests
  - `crates/runner/tests/default_bootstrap_long_lane.rs` — `FK_HEAVY_SOURCE_SETUP_SQL`, `FK_HEAVY_DESTINATION_SETUP_SQL` references
- Remove or rewrite all documentation references in `docs/deepseek_v4_pro_high/`, `docs/glm_5_1/`, `docs/gpt_5_5_medium/`, `docs/kimi_k2_6/` (architecture.md, installation.md, getting-started.md, index.md) that mention setup-sql
- Update `README.md` to remove all setup-sql references

Out of scope:
- Writing the replacement docs (that is Task 02)
- Writing the replacement bash scripts (that is Task 03)
- Changing the runner production code (it does not depend on setup-sql / source_bootstrap at build time)
- Writing any tests to prove the binary was deleted

Under no circumstances should the runner's production functionality be altered. Only test harness code that assumed the setup-sql image/binary exists should be removed.
</description>


<acceptance_criteria>
- [x] `crates/setup-sql/` directory and all its contents are deleted
- [x] Root `Cargo.toml` no longer lists `crates/setup-sql` as a workspace member
- [x] `artifacts/compose/setup-sql.compose.yml` is deleted
- [x] `.github/workflows/image-catalog.yml` no longer contains the setup-sql image entry
- [x] `.github/workflows/publish-images.yml` no longer references `setup_sql_image_ref`
- [x] Root `Dockerfile` no longer copies `crates/setup-sql/Cargo.toml`
- [x] All runner test harness code referencing setup-sql / source_bootstrap / setup-sql compose is removed or adapted
- [x] All documentation files across `docs/*/` and `README.md` no longer reference setup-sql
- [x] Manual verification: `rg -l "setup.sql\|setup-sql\|source_bootstrap" --type-not md` returns no hits outside of `.ralph/tasks/story-28-*`
- [x] Manual verification: `cargo build --workspace` succeeds without the setup-sql crate
- [x] Manual verification: `cargo test --workspace` passes — all remaining tests succeed after removal of setup-sql-dependent test code
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly
- [x] `make lint` — passes cleanly
</acceptance_criteria>

<plan>.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/01-task-remove-setup-sql-binary-and-all-references_plans/2026-04-27-remove-setup-sql-plan.md</plan>
