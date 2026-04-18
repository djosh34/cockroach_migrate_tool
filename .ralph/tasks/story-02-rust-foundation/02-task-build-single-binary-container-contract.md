## Task: Build the single-binary container contract for the destination runner <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Define and implement the destination container contract so the system is one container running one binary that exposes the webhook endpoint and manages PostgreSQL-side state. The higher order goal is to make the runtime shape match production early and avoid script-heavy local-only glue.

In scope:
- Dockerfile for the runner
- direct container startup path
- no wrapper bash scripts as the user path
- binary entrypoint contract
- config file mounting conventions

Out of scope:
- full runtime behavior
- end-to-end migration success

This task must preserve the novice-user constraint from `designs/crdb-to-postgres-cdc/04_operational_model.md`:
- the user path must work directly through Docker commands
- not through helper scripts

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers container build and direct binary startup contract
- [ ] The destination runtime can be built and started directly without wrapper bash scripts
- [ ] The container shape is one binary in one container, matching the selected design
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

