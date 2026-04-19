## Task: Emit the absolute minimum PostgreSQL role grants needed by the runner from the one-time setup image <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Generate the minimum PostgreSQL role-grant SQL required by the runner and emit it from the one-time setup image. The higher order goal is to make destination permissions explicit, least-privileged, and generated once instead of guessed or over-granted later.

In scope:
- output the role grants needed by the runner
- prove the grants are the absolute minimum supported privileges
- tests that fail if extra or overly broad privileges are emitted
- align operator-facing output with the runner's PostgreSQL-only contract
- require only PostgreSQL configuration for this subcommand
- allow the emitted SQL to target a configurable role name
- support both simple JSON output and plain-text SQL-to-stdout output for this command
- keep the JSON schema simple, with one SQL string per destination database
- allow one invocation to emit SQL for multiple configured PostgreSQL databases in a single response, with one SQL string per database in JSON output

Out of scope:
- automatically applying the grants
- granting any source-side permissions

Decisions already made:
- the one-time setup image must output the role grants needed for the runner
- those grants must be the absolute minimum grants needed by the runner
- this subcommand must not require CockroachDB connection details
- the operator must be able to change the role name
- JSON output should be simple, using one SQL string per database
- one response may include multiple PostgreSQL databases, with one SQL string per database in JSON
- plain-text SQL may include human comments, but it does not need machine-readable markers
- PostgreSQL grant output must never be mixed with Cockroach setup output

</description>


<acceptance_criteria>
- [x] Red/green TDD covers generation and least-privilege validation of the runner grant SQL
- [x] The emitted SQL reflects only the minimum PostgreSQL privileges required by the runner, this mode does not require CockroachDB config, and the operator can configure the role name
- [x] The PostgreSQL grant command supports both simple JSON and plain-text SQL output, with simple JSON using one SQL string per destination database and allowing multiple databases in one response
- [x] The output does not assume superuser runtime behavior or broaden the runner's permission scope
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-19-source-sql-emitter-image/03-task-emit-the-absolute-minimum-postgresql-role-grants-needed-by-the-runner_plans/2026-04-19-postgres-minimum-grants-plan.md</plan>
