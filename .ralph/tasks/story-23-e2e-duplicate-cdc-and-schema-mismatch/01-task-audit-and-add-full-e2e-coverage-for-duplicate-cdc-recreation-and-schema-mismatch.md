## Task: Audit full end-to-end coverage for duplicate CDC delivery, recreated feeds, and source-destination schema mismatch, then add any missing cases to the full e2e suite <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Audit whether the existing full end-to-end test suite already covers the key duplicate-delivery and schema-mismatch cases that are realistically going to happen in production-like operation, and add any missing long-lane coverage. The higher order goal is to prove the system behaves safely when duplicate Cockroach changefeeds or replay-like initial scans send the same logical data again, and when source and destination schemas drift out of alignment in a way that could otherwise trigger retry storms or destination-side overload.

In scope:
- inspect the current full e2e coverage and determine whether these scenarios are already covered well enough
- if a scenario is not already covered, add it to the full e2e suite rather than leaving it as an untested assumption
- evaluate what happens when two Cockroach CDC feeds are accidentally active for the same source database and both send to the same destination URL at the same time
- specifically test the case where both feeds repeatedly push the same logical row changes over and over
- determine whether duplicate delivery is harmless because the destination handles duplicates correctly, or whether it breaks correctness, creates unbounded growth, or causes any other bad behavior
- evaluate what happens when one CDC feed is stopped and a new feed is created with `initial_scan = 'yes'` again so historical rows are replayed into the destination path
- determine whether replay from a recreated feed is handled correctly by the ingest plus reconcile design
- evaluate what happens when there is an accidental mismatch between source schema and destination schema for migrated tables
- specifically test whether schema mismatch causes a denial-of-service style retry loop, a huge increase in retries that can hammer the destination database, or a bounded and operator-usable failure mode
- verify ingest-side errors for these cases are logged correctly by the runner with enough information for an operator to diagnose the problem
- record clear conclusions for each scenario rather than leaving the behavior implied

Out of scope:
- broad new chaos cases outside duplicate delivery, feed recreation, and schema mismatch
- changing product behavior preemptively when the new e2e coverage shows the current design is already safe and acceptable

Decisions already made:
- this must be a new story at the end of the backlog rather than an addition inside an older story directory
- the task must first audit current coverage and only add new tests where coverage is missing or insufficient
- these scenarios belong in the full end-to-end test surface, not in a mocked or unit-only substitute
- accidental duplicate CDC feeds targeting the same URL are a realistic failure mode and must be evaluated explicitly
- feed recreation with a fresh `initial_scan = 'yes'` is also a realistic replay mode and must be evaluated explicitly
- schema mismatch behavior must be judged not only on correctness but also on retry pressure and whether it can overload the destination side
- runner ingest errors for these scenarios must be visible in logs clearly enough for operators to understand the failure
- any issue found during this verification that represents a defect, missing guardrail, or unsafe retry pattern must immediately create a bug via the `add-bug` skill
- when a bug is found, the verification flow must ask for a task switch so the system can switch to the bug task
- this task must not be marked passed unless the audit is complete, every missing scenario has coverage, and zero newly discovered defects remain untracked

</description>


<acceptance_criteria>
- [x] Red/green TDD audits existing full e2e coverage for duplicate CDC delivery, recreated-feed replay, and source-destination schema mismatch
- [x] If any of those scenarios are not already covered well enough, the task adds them to the full e2e suite instead of leaving gaps
- [x] The task proves what happens when two CDC feeds for the same source database push to the same destination URL concurrently, including repeated duplicate logical data delivery
- [x] The task proves what happens when a CDC feed is stopped and recreated with `initial_scan = 'yes'` so historical rows are replayed
- [x] The task proves whether schema mismatch creates bounded operator-usable failure or dangerous retry amplification against the destination database
- [x] The task proves runner ingest errors for these scenarios are logged clearly and correctly
- [x] The task records explicit conclusions for each scenario: harmless, bounded-but-needs-operator-action, or defective
- [x] Every issue found during verification immediately results in a new bug task created via `add-bug`, and the workflow asks for a task switch to that bug
- [x] `<passes>true</passes>` is allowed only if the audit is complete, all missing e2e coverage has been added, and no newly discovered issue is left without a bug task
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-23-e2e-duplicate-cdc-and-schema-mismatch/01-task-audit-and-add-full-e2e-coverage-for-duplicate-cdc-recreation-and-schema-mismatch_plans/2026-04-20-duplicate-cdc-schema-mismatch-e2e-plan.md</plan>
