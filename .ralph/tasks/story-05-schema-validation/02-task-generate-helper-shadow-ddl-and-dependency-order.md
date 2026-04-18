## Task: Generate helper shadow DDL and dependency order from the validated schema <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Generate the helper shadow table shape and the dependency order needed for continuous reconcile. The higher order goal is to make the selected design deterministic and automatic instead of hand-maintained.

In scope:
- derive helper shadow tables from validated real tables
- strip serving-oriented structure from shadow tables
- compute parent-before-child and child-before-parent orders
- support composite PKs
- support multiple destination databases

Out of scope:
- webhook runtime
- reconcile execution itself

This task must encode:
- shadow tables keep the data columns
- shadow tables do not keep FKs or secondary indexes
- minimal PK index is allowed only as an automatic runner decision

</description>


<acceptance_criteria>
- [x] Red/green TDD covers helper DDL generation, dependency ordering, and composite-key support
- [x] Generated helper DDL matches the selected shadow-table design rules
- [x] Dependency ordering is explicit and reusable by later reconcile tasks
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-05-schema-validation/02-task-generate-helper-shadow-ddl-and-dependency-order_plans/2026-04-18-helper-shadow-ddl-plan.md</plan>
