## Task: Emit the required Cockroach changefeed SQL from the one-time setup image <status>not_started</status> <passes>false</passes>

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
- [ ] Red/green TDD covers generation of the required Cockroach source-setup SQL
- [ ] The emitted output includes the changefeed creation SQL needed by the supported flow, this mode does not require PostgreSQL config, and it supports both simple JSON and plain-text SQL output
- [ ] JSON output can return multiple configured Cockroach databases in one response, with one SQL string per database
- [ ] The Cockroach command allows configuring webhook URLs and cert paths without mixing in PostgreSQL behavior
- [ ] The runner no longer depends on source-side SQL generation or source access after this step
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
