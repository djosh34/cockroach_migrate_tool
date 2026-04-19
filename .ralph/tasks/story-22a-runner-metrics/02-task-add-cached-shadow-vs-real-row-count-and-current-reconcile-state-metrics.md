## Task: Add cached shadow-versus-real row-count and current reconcile-state metrics <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Extend the runner metrics surface with the smallest additional state needed to show whether transferred rows in the shadow tables match the real destination tables and whether a table is currently red due to reconcile failure. The higher order goal is to make a migration dashboard useful for spotting obvious drift without turning `/metrics` into an expensive scrape-time counting endpoint.

In scope:
- expose current row counts for both the shadow table and the real destination table using the same metric family
- expose current reconcile-error state per destination table
- expose current last-success timestamp per destination table so dashboards can tell whether reconcile is still making forward progress
- ensure row-count export is not implemented as an unbounded expensive `COUNT(*)` fan-out on every scrape
- tests that assert the metric family, layer labels, and refresh behavior contract clearly enough to block accidental scrape-path regressions

Required metric families:
- `cockroach_migration_tool_table_rows{destination_database,destination_table,layer}`
  - gauge
  - `layer` is bounded to `shadow` or `real`
- `cockroach_migration_tool_table_reconcile_error{destination_database,destination_table}`
  - gauge with value `1` when the table currently has a stored reconcile error and `0` otherwise
- `cockroach_migration_tool_reconcile_last_success_unixtime_seconds{destination_database,destination_table}`
  - gauge

Metric and behavior rules already decided:
- every metric name must start with `cockroach_migration_tool_`
- `destination_table` must include schema, for example `public.customers`
- row counts for `layer="shadow"` and `layer="real"` must use the same metric family so dashboards can compare them directly
- row-count mismatch is a useful operational signal, but not a proof of correctness
- row-count export must not make `/metrics` itself the source of large repeated database load; counts must come from a bounded refresh strategy or equivalently safe cached state instead of a full expensive recount on every scrape
- current reconcile error is a separate fact from historical failure counters and should stay explicit as a current-state gauge

Out of scope:
- claiming row-count equality is a full correctness assertion
- broad “healthy” rollup metrics with ambiguous semantics
- runtime source-side counts or source-side lag metrics

Decisions already made:
- shadow-versus-real row counts are important enough to expose even in a minimal dashboard
- the state metric should be specific and factual; use current reconcile error rather than a vague “healthy” metric
- freshness still matters, so a last-success timestamp is worth exporting even though failure counters already exist

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers the `cockroach_migration_tool_table_rows`, `cockroach_migration_tool_table_reconcile_error`, and `cockroach_migration_tool_reconcile_last_success_unixtime_seconds` metric families
- [ ] Dashboards can compare `layer="shadow"` and `layer="real"` row counts for the same schema-qualified destination table directly
- [ ] The `/metrics` endpoint does not execute an unbounded full row-count scan across all tables on every scrape
- [ ] Operators can tell whether a table is currently in reconcile error and when it last reconciled successfully
- [ ] All metric names use the `cockroach_migration_tool_` prefix and all `destination_table` labels are schema-qualified
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<blocked_by>.ralph/tasks/story-22a-runner-metrics/01-task-expose-low-cardinality-runner-activity-timing-and-failure-metrics.md</blocked_by>
