## Bug: Long-lane blocked-reconcile crash recovery still fails during story-23 validation <status>completed</status> <passes>true</passes> <priority>high</priority>

<description>
While validating `.ralph/tasks/story-23-e2e-duplicate-cdc-and-schema-mismatch/01-task-audit-and-add-full-e2e-coverage-for-duplicate-cdc-recreation-and-schema-mismatch.md`, the repo gates reached a remaining blocker in `make test-long`.

What already happened before this bug was isolated:
- `make check` passed
- `make lint` passed
- `make test` passed
- the new duplicate-feed, replay, and schema-mismatch audits were added and their focused runs passed
- verify-service shard summaries were fixed to accumulate across shards, which removed false-zero verification results in older long-lane cases
- `make test-long` was updated to run ignored tests with `--test-threads=1` to remove cross-test interference

What is still broken:
- a clean serialized `make test-long` rerun still hit `ignored_long_lane_recovers_after_runner_crash_during_a_blocked_reconcile_pass`
- that failure blocks the story-23 task from being marked passed, even after the verify-service aggregation fix

The next task should reproduce this specific long-lane failure in isolation, capture the exact assertion/output, and repair either the blocked-reconcile crash recovery behavior or the test harness if the recovery guarantee is being asserted incorrectly.
</description>

<mandatory_red_green_tdd>
Use Red-Green TDD to solve the problem.
You must make ONE test, and then make ONE test green at the time.

Then verify if bug still holds. If yes, create new Red test, and continue with Red-Green TDD until it does work.
</mandatory_red_green_tdd>

<acceptance_criteria>
- [x] I created a Red unit and/or integration test that captures the bug
- [x] I made the test green by fixing
- [x] I manually verified the bug, and created a new Red test if not working still
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this bug impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/bugs/bug-long-lane-crash-recovery-under-blocked-reconcile-still-fails_plans/2026-04-20-reconcile-transaction-failure-recovery-plan.md</plan>
