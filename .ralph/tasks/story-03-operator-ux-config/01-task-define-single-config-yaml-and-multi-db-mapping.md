## Task: Define the single config YAML and multi-database mapping model <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Define the one config YAML used by the destination container, including multiple source-database to destination-database mappings. The higher order goal is to keep the operator interface simple and declarative while still supporting the multi-database requirements of the chosen design.

In scope:
- config YAML schema
- validation rules
- multi-db mapping structure
- webhook TLS config shape
- PostgreSQL connection config shape using scoped credentials only
- reconcile interval config
- MOLT verify integration config needed later

Out of scope:
- README polish
- implementation of full runtime behavior

The design requires one destination container that can control multiple source and destination databases from one config. This task must make that shape explicit and testable.

</description>


<acceptance_criteria>
- [x] Red/green TDD covers config parsing, validation, and multi-db mapping behavior
- [x] One YAML config can describe the single destination container and multiple source-to-destination mappings
- [x] Scoped PostgreSQL role configuration is represented directly in config without superuser assumptions
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-03-operator-ux-config/01-task-define-single-config-yaml-and-multi-db-mapping_plans/2026-04-18-single-config-yaml-and-multi-db-mapping-plan.md</plan>
