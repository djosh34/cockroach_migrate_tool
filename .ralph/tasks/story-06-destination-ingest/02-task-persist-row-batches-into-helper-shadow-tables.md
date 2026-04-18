## Task: Persist row batches idempotently into helper shadow tables <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Persist incoming CDC row batches into the `_cockroach_migration_tool` shadow tables with idempotent behavior and correct `200` semantics. The higher order goal is to make webhook success mean durable helper-state persistence and nothing weaker or stronger.

In scope:
- parse row batches
- map rows to target helper shadow tables
- apply inserts, updates, and deletes into helper shadow state
- support composite PKs
- return `200` only after the PostgreSQL transaction commits
- support duplicate deliveries safely

Out of scope:
- real-table reconcile
- resolved watermark tracking beyond what is necessary for this task

This task must preserve the selected rule:
- webhook success is about durable helper-state persistence into PostgreSQL migration tables

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers insert, update, delete, duplicate-delivery, and composite-PK helper persistence cases
- [ ] HTTP `200` is returned only after helper-state persistence commits successfully
- [ ] Duplicate row-batch delivery does not corrupt helper shadow state
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

