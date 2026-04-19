## Task: Expose verify job progress and result metrics from the HTTP verify service <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Add a `/metrics` endpoint to the verify service so operators and tests can observe progress and outcomes for running verification jobs. The higher order goal is to make remote verification transparent enough to diagnose long-running or failing comparisons without special log scraping.

In scope:
- `/metrics` endpoint
- all verify metrics prefixed with `cockroach_migration_tool_verify_`
- per-job labels including `job_id`
- metrics that expose per source database and per source table row counts
- metrics that expose per destination database and per destination table row counts
- metrics that expose checked row counts
- metrics that expose mismatch counts
- metrics that expose error counts
- metric naming and labels that make `rows todo` implied from source-versus-destination counts rather than exported as a separate vague metric
- tests that assert metric correctness and cardinality choices explicitly

Out of scope:
- external dashboarding, which will be done later in another task

Decisions already made:
- metrics must expose progress of verification
- every verify metric must use the `cockroach_migration_tool_verify_` prefix
- metrics must include a label per verification job named `job_id`
- results should include source row counts, destination row counts, checked rows, mismatches, and errors rather than only binary success/failure
- `rows todo` should not be exported as its own metric when it can be derived from clearer source-versus-destination count metrics

</description>


<acceptance_criteria>
 - [x] Red/green TDD covers `/metrics` exposure and validates the required per-job progress/result metrics
 - [x] Every exported verify metric uses the `cockroach_migration_tool_verify_` prefix
 - [x] Metrics include a `job_id` label and expose per source database/table row counts, per destination database/table row counts, checked rows, mismatches, and errors
 - [x] Operators can observe running and completed verification jobs without scraping ad-hoc log text
 - [x] `make check` — passes cleanly
 - [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
 - [x] `make lint` — passes cleanly
 - [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
