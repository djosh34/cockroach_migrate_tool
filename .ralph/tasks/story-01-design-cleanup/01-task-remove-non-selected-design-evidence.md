## Task: Remove non-selected design evidence from the design package <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Make the design package under `designs/crdb-to-postgres-cdc/` describe only the selected shadow-table migration design. Remove any remaining references to alternative architecture branches, abandoned acknowledgement rules, generic event-log merge-engine preference, or competing design comparisons. The higher order goal is to ensure the implementation backlog starts from one single design truth instead of carrying ambiguous old branches into execution.

In scope:
- audit all design files under `designs/crdb-to-postgres-cdc/`
- remove any leftover evidence of non-selected designs
- keep only the chosen model:
  - one destination container
  - one binary
  - helper schema `_cockroach_migration_tool`
  - one shadow table per real table
  - webhook `200` only after durable helper-state persistence
  - continuous reconcile into the real constrained tables
  - API-level write-freeze cutover
- ensure the design files state that MOLT verify checks the real target tables, not the shadow tables

Out of scope:
- implementation code
- task creation beyond keeping design references accurate

This task must leave the design package aligned with:
- `designs/crdb-to-postgres-cdc/02_requirements.md`
- `designs/crdb-to-postgres-cdc/03_shadow_table_architecture.md`
- `designs/crdb-to-postgres-cdc/04_operational_model.md`
- `designs/crdb-to-postgres-cdc/05_design_decisions.md`
- `designs/crdb-to-postgres-cdc/06_recommended_design.md`
- `designs/crdb-to-postgres-cdc/07_test_strategy.md`

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers a design-package consistency check or equivalent repo-level assertion that no old alternative-design markers remain
- [ ] The design files describe only the selected shadow-table design
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
