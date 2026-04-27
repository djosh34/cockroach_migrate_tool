# Plan: Remove The Obsolete setup-sql Crate And Collapse Its Test/Doc Boundaries

## References

- Task:
  - `.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/01-task-remove-setup-sql-binary-and-all-references.md`
- Workspace and build roots:
  - `Cargo.toml`
  - `Cargo.lock`
  - `Dockerfile`
- setup-sql crate and compose artifact to delete:
  - `crates/setup-sql/`
  - `artifacts/compose/setup-sql.compose.yml`
- Workflow and publish surfaces:
  - `.github/workflows/image-catalog.yml`
  - `.github/workflows/publish-images.yml`
  - `.github/workflows/promote-image-tags.yml`
- Runner test harness surfaces currently coupled to setup-sql:
  - `crates/runner/tests/support/published_image_refs.rs`
  - `crates/runner/tests/support/operator_cli_surface.rs`
  - `crates/runner/tests/support/novice_registry_only_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/multi_mapping_harness.rs`
  - `crates/runner/tests/support/composite_pk_exclusion_harness.rs`
  - `crates/runner/tests/support/readme_operator_workspace.rs`
  - `crates/runner/tests/support/rust_workspace_image_cache_contract.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
  - `crates/runner/tests/operator_cli_surface_contract.rs`
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/runner/tests/readme_operator_surface_contract.rs`
- Documentation surfaces currently advertising setup-sql:
  - `README.md`
  - `docs/deepseek_v4_pro_high/architecture.md`
  - `docs/deepseek_v4_pro_high/getting-started.md`
  - `docs/deepseek_v4_pro_high/installation.md`
  - `docs/glm_5_1/architecture.md`
  - `docs/glm_5_1/getting-started.md`
  - `docs/glm_5_1/installation.md`
  - `docs/gpt_5_5_medium/architecture.md`
  - `docs/gpt_5_5_medium/getting-started.md`
  - `docs/gpt_5_5_medium/index.md`
  - `docs/gpt_5_5_medium/installation.md`
  - `docs/kimi_k2_6/architecture.md`
  - `docs/kimi_k2_6/getting-started.md`
  - `docs/kimi_k2_6/installation.md`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for this planning turn.
- This turn is planning-only because the task had no linked plan artifact yet.
- This is greenfield cleanup:
  - delete the obsolete crate and every direct reference to it
  - do not preserve compatibility wrappers, aliases, or placeholder workflow outputs
  - remove stale docs rather than soft-deprecating them
- The task explicitly defines this work as pure deletion/cleanup with no new tests.
  - the honest RED signal here is failing build/test/lint and failing repository-wide reference scans
  - execution will still use the `tdd` skill mindset: make one coherent slice of removal at a time, then run the real public verification lanes
- If execution reveals that runner production code, not only tests/docs/workflows, still depends on setup-sql at build time or runtime, this plan is wrong and must be switched back to `TO BE VERIFIED` immediately.
- If execution reveals that removing setup-sql requires inventing replacement docs or replacement scripts now instead of completing a deletion-only cleanup, this plan is wrong and must be switched back to `TO BE VERIFIED` immediately.

## Current State Summary

- `setup-sql` is still a first-class Rust workspace member and its package remains locked in `Cargo.lock`.
- The root `Dockerfile` and a runner cache-contract test still treat `crates/setup-sql/Cargo.toml` as part of the workspace image build seam.
- The runner production crate does not appear to import setup-sql directly.
  - the coupling is concentrated in test harnesses, novice workspace fixtures, long-lane bootstrap helpers, and docs
- The deepest leftover boundary smell is in the runner test support layer:
  - test harness code still treats `setup-sql` as if it were part of the runner-owned operator contract
  - harnesses generate source bootstrap SQL by invoking `cargo_bin("setup-sql")` and by copying setup-sql fixtures into novice workspaces
  - that spreads a deleted tool's contract across multiple unrelated test helpers
- The README and generated docs still present a three-image operator story even though this story is explicitly moving toward docs plus bash scripts instead of a published setup-sql binary.

## Improve-Code-Boundaries Focus

- Primary boundary problem to flatten:
  - runner test infrastructure currently owns behavior that belongs to the soon-to-be-removed setup-sql tool boundary
- Required cleanup during execution:
  - delete setup-sql-specific image-reference helpers and CLI surface helpers instead of leaving dead stubs
  - remove setup-sql-driven bootstrap rendering from runner harnesses and collapse those harnesses onto direct SQL/setup data that actually belongs to the tests
  - remove novice-workspace materialization that copies setup-sql fixtures or compose files
  - remove documentation and workflow language that still models setup-sql as a shipped image/binary
- Bold refactor allowance:
  - if a support module only exists to serve setup-sql-oriented tests, delete the entire file
  - if a contract test only proves setup-sql surfaces, delete the whole test instead of rewriting it into a weaker string check
  - if a helper struct field exists only to thread setup-sql-generated source bootstrap through the harness, remove the field and simplify the constructor API

## Public Verification Strategy

- No new tests will be added for deletion-only work.
- Execution will prove correctness through the existing repo-wide public lanes:
  - `cargo build --workspace`
  - `cargo test --workspace`
  - `make check`
  - `make lint`
  - `make test`
- Repository scan gates:
  - `rg -l "setup.sql|setup-sql|source_bootstrap" --type-not md`
  - targeted follow-up scans for `setup_sql_image_ref`, `source_setup_sql`, and `destination_setup_sql`
- The long/e2e lane remains out of scope for this task unless the task definition is proven incomplete.

## Intended Files And Structure To Change

- Delete:
  - `crates/setup-sql/`
  - `artifacts/compose/setup-sql.compose.yml`
- Update workspace/build metadata:
  - `Cargo.toml`
  - `Cargo.lock`
  - `Dockerfile`
- Update workflow/image publishing surfaces:
  - `.github/workflows/image-catalog.yml`
  - `.github/workflows/publish-images.yml`
  - `.github/workflows/promote-image-tags.yml` only if it still names setup-sql explicitly after inspection
- Update or delete runner support files that only exist for setup-sql:
  - `crates/runner/tests/support/published_image_refs.rs`
  - `crates/runner/tests/support/operator_cli_surface.rs`
  - `crates/runner/tests/support/novice_registry_only_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/default_bootstrap_harness.rs`
  - `crates/runner/tests/support/multi_mapping_harness.rs`
  - `crates/runner/tests/support/composite_pk_exclusion_harness.rs`
  - `crates/runner/tests/support/readme_operator_workspace.rs`
  - `crates/runner/tests/support/rust_workspace_image_cache_contract.rs`
  - `crates/runner/tests/support/e2e_integrity.rs`
- Update or delete runner contract tests that only assert setup-sql surfaces:
  - `crates/runner/tests/operator_cli_surface_contract.rs`
  - `crates/runner/tests/novice_registry_only_contract.rs`
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/default_bootstrap_long_lane.rs`
- Update docs:
  - `README.md`
  - the `docs/*/architecture.md`, `getting-started.md`, `installation.md`, and `index.md` files listed above

## Execution Slices

### Slice 1: Remove The Crate From The Workspace And Build Surfaces

- Delete `crates/setup-sql/` and remove it from:
  - `Cargo.toml`
  - `Cargo.lock`
  - root `Dockerfile`
- Inspect `.github/workflows/image-catalog.yml`, `publish-images.yml`, and `promote-image-tags.yml` and remove any setup-sql image/output wiring.
- RED signal:
  - `cargo build --workspace` or workflow-related lint/build checks fail because setup-sql references still exist
- GREEN:
  - the workspace builds again without any setup-sql package/member/image catalog entry
- REFACTOR:
  - remove any now-dead helper values or workflow outputs instead of leaving empty plumbing

### Slice 2: Collapse Runner Test Harnesses Off The Deleted Tool Boundary

- Remove `setup_sql_image_ref()` and any CLI surface helpers for `setup-sql`.
- Refactor harnesses that currently render source bootstrap SQL through `cargo_bin("setup-sql")`.
  - keep only the direct SQL/setup inputs that the runner tests genuinely own
  - remove `source_setup_sql`, `destination_setup_sql`, `source_bootstrap_*`, and related pass-through state wherever it only existed to serve setup-sql
- Delete contract tests that only validate the removed setup-sql compose/image/operator surface.
- RED signal:
  - `cargo test --workspace` fails because test support still imports deleted symbols, files, or compose artifacts
- GREEN:
  - remaining tests compile and pass with no setup-sql helper seam left in runner support
- REFACTOR:
  - merge or delete support modules whose only job was to carry setup-sql bootstrap data through the test harness

### Slice 3: Remove Doc And README References Without Inventing Replacement Content

- Rewrite `README.md` and generated docs so they no longer claim a setup-sql binary/image/compose artifact exists.
- Keep this task deletion-only:
  - remove stale instructions
  - do not pre-implement Task 02 or Task 03 replacement docs/scripts beyond minimal wording needed to keep docs accurate
- RED signal:
  - repository scan still finds setup-sql references outside the task files
- GREEN:
  - docs describe only surfaces that actually exist after deletion
- REFACTOR:
  - if a doc section exists only to explain setup-sql, remove the full section instead of leaving broken transitional prose

### Slice 4: Full Validation And Mud Check

- Run:
  - `cargo build --workspace`
  - `cargo test --workspace`
  - `make check`
  - `make lint`
  - `make test`
- Run reference scans again to prove the obsolete boundary is gone.
- Final mud check using `improve-code-boundaries`:
  - confirm no support module, workflow output, or doc section still exists solely to route through a deleted setup-sql seam
- If validation shows the harness still needs a replacement abstraction rather than pure removal, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Done Condition

- The setup-sql crate, image artifacts, compose artifact, workflow references, test-harness seams, and docs are all removed cleanly.
- The remaining runner production code surface is unchanged.
- The repo passes `make check`, `make lint`, and `make test`.
- The repo contains no non-task references to `setup-sql`, `setup_sql`, or `source_bootstrap` that imply the deleted tool still exists.

Plan path: `.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/01-task-remove-setup-sql-binary-and-all-references_plans/2026-04-27-remove-setup-sql-plan.md`

NOW EXECUTE
