## Task: Expose low-cardinality runner activity, timing, and failure metrics at `/metrics` <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Add a simple `/metrics` endpoint to the runner that exposes only the minimum low-cardinality metrics needed to tell whether migration traffic is arriving, whether apply/reconcile work is still happening, how long that work takes, and whether it is failing. The higher order goal is to make a small migration dashboard possible without log scraping and without blasting the metrics backend with a large label surface.

In scope:
- expose `/metrics` from the runner
- use only metrics prefixed with `cockroach_migration_tool_`
- keep labels intentionally small and bounded
- expose webhook activity metrics so operators can answer:
  - is the webhook being called at all
  - how many requests arrived in the last 30 minutes
  - when was the latest request
- expose apply and reconcile timing metrics so operators can answer:
  - how long did it take to apply changes into the shadow table
  - how long did it take to copy shadow-table state into the real table
- expose failure metrics and latest-attempt timestamps so operators can answer:
  - did the runner keep trying
  - did the latest attempt succeed or fail
  - how many application failures happened over a window
- tests that assert metric names, labels, types, and representative output exactly enough to keep the surface stable

Required metric families:
- `cockroach_migration_tool_webhook_requests_total{destination_database,kind,outcome}`
  - counter
  - `kind` is bounded to `row_batch` or `resolved`
  - `outcome` is bounded to `ok`, `bad_request`, or `internal_error`
- `cockroach_migration_tool_webhook_last_request_unixtime_seconds{destination_database}`
  - gauge
- `cockroach_migration_tool_webhook_apply_duration_seconds_total{destination_database,destination_table}`
  - counter
- `cockroach_migration_tool_webhook_apply_requests_total{destination_database,destination_table}`
  - counter
- `cockroach_migration_tool_webhook_apply_last_duration_seconds{destination_database,destination_table}`
  - gauge
- `cockroach_migration_tool_reconcile_apply_duration_seconds_total{destination_database,destination_table,phase}`
  - counter
  - `phase` is bounded to `upsert` or `delete`
- `cockroach_migration_tool_reconcile_apply_attempts_total{destination_database,destination_table,phase}`
  - counter
- `cockroach_migration_tool_reconcile_apply_last_duration_seconds{destination_database,destination_table,phase}`
  - gauge
- `cockroach_migration_tool_apply_failures_total{destination_database,destination_table,stage}`
  - counter
  - `stage` is bounded to `webhook_apply`, `reconcile_upsert`, or `reconcile_delete`
- `cockroach_migration_tool_apply_last_outcome_unixtime_seconds{destination_database,destination_table,stage,outcome}`
  - gauge
  - `outcome` is bounded to `success` or `error`

Metric and label rules already decided:
- every metric name must start with `cockroach_migration_tool_`
- use counters for “how many over time” questions so dashboards can ask for the last 30 minutes with window functions
- use gauges for latest timestamps and latest single-attempt duration
- use seconds for apply and reconcile duration metrics, including fractional seconds for sub-second work
- use a cumulative duration counter together with a cumulative request/attempt counter so dashboards can compute average duration over a time window
- do not use histograms in this first version
- `destination_table` must include schema, for example `public.customers`
- do not use raw error messages, raw request paths, request ids, helper-table names, or raw watermark strings as labels
- the runner cannot talk to source CockroachDB at runtime, so no metric may require runtime source access

Out of scope:
- row-count metrics for shadow versus real tables
- scrape-time expensive counting across all tables
- dashboard or alert configuration outside the runner itself

Decisions already made:
- the first runner metrics cut must stay minimal and directly useful
- low cardinality is more important than exposing every possible internal state
- latest outcome timestamps must distinguish success from error so operators can see repeated failing retries
- timing metrics should follow Prometheus conventions and use fractional seconds rather than nanoseconds
- request counts should be exposed with `_total` counters; cumulative duration should be exposed as `_seconds_total` counters rather than histogram-style buckets in this first version

</description>


<acceptance_criteria>
- [x] Red/green TDD covers `/metrics` exposure and validates the required metric names, labels, and types
- [x] Operators can answer “is the webhook being called”, “how many requests arrived recently”, and “when was the latest request” from the exported metrics alone
- [x] Operators can compute average webhook-apply and reconcile-apply duration over a time window without histograms
- [x] Operators can tell when success last happened, when error last happened, and whether failures are accumulating over time
- [x] All metric names use the `cockroach_migration_tool_` prefix and all `destination_table` labels are schema-qualified
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] `make test-long` — not applicable; this task did not change the ultra-long / e2e lane boundary
</acceptance_criteria>

<plan>.ralph/tasks/story-22a-runner-metrics/01-task-expose-low-cardinality-runner-activity-timing-and-failure-metrics_plans/2026-04-20-runner-activity-metrics-plan.md</plan>
