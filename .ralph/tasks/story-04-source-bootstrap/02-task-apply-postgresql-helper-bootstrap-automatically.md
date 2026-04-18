## Task: Apply helper-schema bootstrap inside PostgreSQL automatically from the runner <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Implement the runner-side PostgreSQL bootstrap that creates and prepares `_cockroach_migration_tool` and its helper tables automatically. The higher order goal is to reduce manual operator work while keeping grants explicit and separate.

In scope:
- create helper schema
- create tracking tables
- create helper shadow tables when schema validation passes
- automatic creation of a minimal primary-key index on helper shadow tables when the runner decides it is needed

Out of scope:
- role grants
- source bootstrap

This task must preserve:
- helper schema lives inside each destination database
- shadow tables are for ingest, not serving
- no operator-managed index toggles

</description>


<acceptance_criteria>
 - [x] Red/green TDD covers helper bootstrap, repeatability, and automatic shadow-table preparation
 - [x] The runner can bootstrap `_cockroach_migration_tool` and helper shadow tables without extra manual PostgreSQL scripting beyond grants
 - [x] Minimal helper PK indexing, when used, is automatic rather than operator-managed
 - [x] `make check` — passes cleanly
 - [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
 - [x] `make lint` — passes cleanly
 - [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-04-source-bootstrap/02-task-apply-postgresql-helper-bootstrap-automatically_plans/2026-04-18-postgresql-helper-bootstrap-plan.md</plan>
