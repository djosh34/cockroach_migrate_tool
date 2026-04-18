## Task: Generate PostgreSQL grant SQL and operator-facing bootstrap artifacts <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Generate the SQL artifacts and operator outputs needed to prepare PostgreSQL for the runner without assuming superuser use by the runtime container. The higher order goal is to separate what must be granted manually from what the runner can bootstrap automatically.

In scope:
- SQL files or generated SQL for required scoped grants
- helper schema ownership/grant contract
- operator-readable output describing required PostgreSQL setup
- mapping from config to grant requirements

Out of scope:
- automatic execution of grants by the runtime
- source-side Cockroach bootstrap

This task must preserve the chosen rule:
- the runner applies what it can inside PostgreSQL automatically later
- but role grants remain explicit SQL artifacts

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers SQL generation from config and required privilege assertions
- [ ] Generated artifacts describe only scoped-role needs and avoid superuser assumptions for runtime behavior
- [ ] The helper schema grant contract is explicit and reproducible
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

