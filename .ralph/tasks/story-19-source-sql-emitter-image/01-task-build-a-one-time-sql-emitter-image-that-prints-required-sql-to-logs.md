## Task: Build a one-time setup image that prints all required SQL to logs instead of executing bash scripts <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Create the operator-facing one-time setup image whose only job is to emit the required SQL in one of two explicit output formats. The higher order goal is to let a developer generate the exact setup SQL once without granting the runtime runner image extra powers or depending on local scripts.

In scope:
- a dedicated image for one-time setup only
- image runtime contract that prints required SQL to logs
- tests that prove it does not depend on bash-first output semantics
- documentation and examples aligned to the log-output contract
- two explicit subcommands so one invocation can target CockroachDB-only work and another can target PostgreSQL-only work
- two output formats per subcommand: simple JSON and plain text that dumps SQL to stdout

Out of scope:
- applying the emitted SQL automatically
- runner runtime behavior

Decisions already made:
- this image is used once by a developer/operator to generate SQL
- it is not bash based anymore
- its contract is outputting the right SQL in logs
- the image must expose two subcommands rather than one combined mode
- one subcommand needs only CockroachDB connectivity
- the other subcommand needs only PostgreSQL connectivity
- the two commands must never be mixed into one combined output
- JSON output should stay simple
- plain text mode should dump SQL to stdout

</description>


<acceptance_criteria>
- [x] Red/green TDD covers running the setup image and capturing the required SQL from logs
- [x] The setup image emits SQL only, does not require a generated shell script as the supported interface, exposes separate Cockroach-only and PostgreSQL-only subcommands, and supports simple JSON plus plain-text SQL output modes
- [x] The image is clearly separated from runner and verify runtime concerns
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-19-source-sql-emitter-image/01-task-build-a-one-time-sql-emitter-image-that-prints-required-sql-to-logs_plans/2026-04-19-one-time-setup-image-plan.md</plan>
