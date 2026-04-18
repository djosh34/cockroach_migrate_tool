## Task: Persist resolved watermarks and stream state <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Persist resolved messages and stream tracking state in `_cockroach_migration_tool`. The higher order goal is to make restartability and reconcile progress explicit and queryable with a small, direct state model.

In scope:
- persist latest received resolved watermark
- persist source stream metadata
- track latest reconciled watermark placeholder fields
- support multi-db mappings
- commit resolved updates transactionally before `200`

Out of scope:
- full reconcile logic

This task must align with the selected minimal tracking model:
- `stream_state`
- `table_sync_state`

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers resolved-message persistence, stream-state updates, and restart-state reads
- [ ] Stream tracking state is durable and queryable in PostgreSQL helper tables
- [ ] Resolved messages return `200` only after their tracking transaction commits
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
