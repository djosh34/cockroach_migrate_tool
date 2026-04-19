## Task: Remove all runner access to the source CockroachDB and delete the related config surface <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Remove every runtime and configuration path that allows the runner container to connect to, read from, or otherwise depend on the original CockroachDB source database. The higher order goal is to hard-split the system into separate images so the runner can be deployed in environments where source-database access is impossible.

In scope:
- delete runner code paths that connect to CockroachDB
- delete runner config fields, parsing, validation, environment wiring, docs, and tests that imply source access from the runner
- remove any runner behavior that reads source schema, source rows, or source verification state
- add tests that fail if the runner binary regains any source-database dependency

Out of scope:
- building the new verify image itself
- building the new SQL-emitter image itself

Decisions already made:
- the runner scratch image must only connect to PostgreSQL
- no backwards-compatibility layer is allowed
- legacy source-access hooks must be removed, not deprecated

</description>


<acceptance_criteria>
- [x] Red/green TDD covers removal of runner source-database access and config
- [x] The runner binary and config contract contain no CockroachDB/source connection settings
- [x] Automated checks prove the runner cannot read or verify against the source database anymore
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-16-runtime-split-removals/01-task-remove-runner-source-cockroach-access-and-config_plans/2026-04-19-runner-source-access-removal-plan.md</plan>
