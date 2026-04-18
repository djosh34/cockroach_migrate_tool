## Task: Persist row batches idempotently into helper shadow tables <status>done</status> <passes>true</passes>

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
- [x] Red/green TDD covers insert, update, delete, duplicate-delivery, and composite-PK helper persistence cases
- [x] HTTP `200` is returned only after helper-state persistence commits successfully
- [x] Duplicate row-batch delivery does not corrupt helper shadow state
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-06-destination-ingest/02-task-persist-row-batches-into-helper-shadow-tables_plans/2026-04-18-row-batch-helper-persistence-plan.md</plan>
