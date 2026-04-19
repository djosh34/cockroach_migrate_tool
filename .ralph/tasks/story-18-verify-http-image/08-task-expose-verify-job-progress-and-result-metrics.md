## Task: Expose verify job progress and result metrics from the HTTP verify service <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Add a `/metrics` endpoint to the verify service so operators and tests can observe progress and outcomes for running verification jobs. The higher order goal is to make remote verification transparent enough to diagnose long-running or failing comparisons without special log scraping.

In scope:
- `/metrics` endpoint
- per-job labels including `job_id`
- metrics for rows todo, transferred/checked, total, error count, mismatch count, and equivalent verify progress counters available from the implementation
- tests that assert metric correctness and cardinality choices explicitly

Out of scope:
- external dashboarding

Decisions already made:
- metrics must expose progress of verification
- metrics must include a label per verification job named `job_id`
- results should include row and mismatch/error visibility rather than only binary success/failure

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers `/metrics` exposure and validates the required per-job progress/result metrics
- [ ] Metrics include a `job_id` label and expose job progress, totals, mismatches, and errors
- [ ] Operators can observe running and completed verification jobs without scraping ad-hoc log text
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
