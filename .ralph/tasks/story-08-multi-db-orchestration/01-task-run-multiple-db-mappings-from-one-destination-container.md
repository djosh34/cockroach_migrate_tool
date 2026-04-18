## Task: Run multiple database mappings from one destination container <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Support multiple source-database to destination-database mappings controlled by one destination container and one binary. The higher order goal is to satisfy the production shape where one runner container owns the webhook endpoint and the PostgreSQL-side apply flow for many migrations at once.

In scope:
- isolate per-database helper schema state
- route row events to the correct destination database
- route reconcile work to the correct destination database
- keep stream state separate per mapping

Out of scope:
- specific E2E scenarios
- README-only novice-user validation

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers multiple configured database mappings in one runtime
- [ ] One destination container can ingest and reconcile more than one source-to-destination mapping safely
- [ ] Helper state remains isolated per destination database
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

