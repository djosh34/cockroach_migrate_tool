## Task: Define the single config YAML and multi-database mapping model <status>not_started</status> <passes>false</passes>

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
- [ ] Red/green TDD covers config parsing, validation, and multi-db mapping behavior
- [ ] One YAML config can describe the single destination container and multiple source-to-destination mappings
- [ ] Scoped PostgreSQL role configuration is represented directly in config without superuser assumptions
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

