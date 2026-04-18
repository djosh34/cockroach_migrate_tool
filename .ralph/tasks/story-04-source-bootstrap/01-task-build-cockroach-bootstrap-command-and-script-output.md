## Task: Build the Cockroach bootstrap command and emitted setup script <status>in_progress</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Build the source-side bootstrap command that produces the CockroachDB setup needed to start CDC on a default or naked source database. The higher order goal is to make source setup explicit, scriptable, and runnable in a pipeline without requiring the operator to manually invent commands.

In scope:
- command that captures the source cursor
- command output or generated script that creates the changefeed(s)
- required Cockroach cluster/database setting changes documented as executable script output
- selected table filtering
- multi-database support

Out of scope:
- destination-side runtime
- repeated source-side intervention after setup

This task must reflect the hard E2E rule:
- after CDC setup is completed once, the tests and production flow are not allowed to keep issuing extra source-side commands just to make the system work

</description>


<acceptance_criteria>
- [x] Red/green TDD covers source bootstrap command generation and required-setting output
- [x] The bootstrap output makes all required Cockroach changes explicit for a default source
- [x] The command supports multi-database CDC setup from one operator flow
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-04-source-bootstrap/01-task-build-cockroach-bootstrap-command-and-script-output_plans/2026-04-18-cockroach-bootstrap-command-and-script-output-plan.md</plan>
