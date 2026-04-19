## Task: Document the API write-freeze cutover runbook <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Document the chosen cutover flow so an operator can execute it correctly with minimal steps. The higher order goal is to turn the selected write-freeze model into a simple, repeatable, README-backed operational path.

In scope:
- document repeated parity checks
- document API write freeze
- document drain-to-zero wait
- document final MOLT verify
- document switch criteria

Out of scope:
- generic migration theory

</description>


<acceptance_criteria>
- [x] Red/green TDD covers documentation assertions or README/runbook checks as appropriate
- [x] The runbook matches the selected cutover model exactly
- [x] The runbook is concise and directly actionable
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-09-verification-cutover/03-task-document-api-write-freeze-cutover-runbook_plans/2026-04-19-api-write-freeze-cutover-runbook-plan.md</plan>
