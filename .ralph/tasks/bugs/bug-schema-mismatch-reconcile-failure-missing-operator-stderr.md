## Bug: Schema mismatch reconcile failure is persisted but not logged to stderr for operators <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
Schema mismatch verification in story 23 task 01 exposed a real operator-surface defect.

When the destination schema drifts out of alignment after bootstrap, the runner does the bounded part correctly:
- webhook ingest still returns `200` after helper-table persistence
- helper shadow state advances to the latest row
- reconcile stalls at the last good watermark instead of retrying ingress
- `_cockroach_migration_tool.table_sync_state.last_error` is populated

But the runner emits no operator-visible stderr context at all for the reconcile failure. The long-lane scenario `ignored_long_lane_classifies_customer_schema_mismatch_through_a_typed_audit` currently observes empty stderr while the failure is happening.

This violates the task requirement that these failure modes be visible and diagnosable for operators, not only persisted in tracking tables.

In scope:
- add red coverage proving schema mismatch reconcile failure emits operator-visible stderr context
- make the runtime log the reconcile failure with mapping id, table, and database error detail while preserving the bounded failure behavior
- keep the persisted `last_error` behavior intact
- prove there is still no ingress retry amplification for the schema mismatch path

Out of scope:
- changing duplicate-feed or recreated-feed replay behavior, which already verified as harmless in the long lane
- weakening the long-lane audit to ignore missing logs
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I created a Red test that proves schema mismatch reconcile failure currently lacks operator-visible stderr logging
- [x] I made the test green by logging reconcile failure context without changing the bounded failure semantics
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] The long-lane schema mismatch audit proves `last_error` persistence and stderr logging together
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] The required ignored schema-mismatch long-lane proof passed, so a full `make test-long` run was not required for this task
</acceptance_criteria>

<plan>.ralph/tasks/bugs/bug-schema-mismatch-reconcile-failure-missing-operator-stderr_plans/2026-04-20-reconcile-failure-stderr-plan.md</plan>
