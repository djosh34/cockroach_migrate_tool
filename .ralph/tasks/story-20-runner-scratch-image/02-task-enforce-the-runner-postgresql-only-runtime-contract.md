## Task: Enforce the runner PostgreSQL-only runtime contract and prove it cannot access source or verify responsibilities <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add hard enforcement around the runner contract so it can connect only to PostgreSQL and cannot perform source-side or verify-side responsibilities. The higher order goal is to prevent architectural drift after the three-image split is implemented.

In scope:
- contract tests around allowed runner configuration and network targets
- enforcement that the runner does not read from CockroachDB
- enforcement that the runner does not perform verify work
- CI/test coverage that fails on boundary regressions

Out of scope:
- implementing the verify image internals
- implementing source setup SQL generation

Decisions already made:
- runner must never access source CockroachDB
- runner must never do verify
- runner must only connect to PostgreSQL

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers the PostgreSQL-only runner contract and boundary-regression failures
- [ ] Tests fail if the runner regains source-database access or verify behavior
- [ ] The runner runtime contract is explicit enough to block future boundary creep
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-20-runner-scratch-image/02-task-enforce-the-runner-postgresql-only-runtime-contract_plans/2026-04-19-runner-postgresql-only-runtime-contract-plan.md</plan>
