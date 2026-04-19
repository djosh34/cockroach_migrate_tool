## Task: Emit the required Cockroach changefeed SQL from the one-time setup image <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Generate the exact CockroachDB SQL needed for source-side changefeed setup from the one-time setup image. The higher order goal is to make source-side enablement explicit and reproducible without requiring the runner to access or mutate the source cluster later.

In scope:
- output the required Cockroach changefeed creation SQL
- output any other required Cockroach SQL/settings changes that are part of the supported source-setup contract
- tests that prove the emitted SQL is correct and complete for the supported setup modes
- require only CockroachDB configuration for this subcommand
- allow configuration of webhook URLs and certificate paths needed by the Cockroach-side setup
- support both simple JSON output and plain-text SQL-to-stdout output for this command
- ensure plain-text SQL includes human comments while JSON stays machine-friendly and simple
- allow one invocation to emit SQL for multiple configured Cockroach databases in a single response, with one SQL string per database in JSON output

Out of scope:
- executing the SQL against Cockroach automatically
- runner behavior after setup

Decisions already made:
- the setup image must output the SQL needed for Cockroach changefeed creation
- source-side access happens only in this setup phase, not in the runner
- this subcommand must not require PostgreSQL connection details
- JSON output should be simple and should not mix Cockroach and PostgreSQL work together
- one response may include multiple Cockroach databases, but each database maps to one SQL string in JSON
- plain-text SQL may include comments, but it does not need machine-readable markers
- Cockroach setup should stay very simple while still allowing webhook URL and cert-path customization

</description>


<acceptance_criteria>
- [x] Red/green TDD covers generation of the required Cockroach source-setup SQL
- [x] The emitted output includes the changefeed creation SQL needed by the supported flow, this mode does not require PostgreSQL config, and it supports both simple JSON and plain-text SQL output
- [x] JSON output can return multiple configured Cockroach databases in one response, with one SQL string per database
- [x] The Cockroach command allows configuring webhook URLs and cert paths without mixing in PostgreSQL behavior
- [x] The runner no longer depends on source-side SQL generation or source access after this step
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-19-source-sql-emitter-image/02-task-emit-the-required-cockroach-changefeed-sql-from-the-one-time-setup-image_plans/2026-04-19-cockroach-changefeed-sql-plan.md</plan>
