## Task: Assert single-container, TLS, and scoped-role integrity in end-to-end tests <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Build explicit verification that end-to-end tests use the real intended runtime shape: one destination container, real TLS on HTTP, real CockroachDB, and a scoped PostgreSQL role only. The higher order goal is to prevent the test environment from quietly drifting into a stronger or simpler environment than production.

In scope:
- assert one destination container manages webhook and PostgreSQL apply
- assert HTTP uses TLS
- assert CockroachDB is real
- assert PostgreSQL runtime role is scoped and not superuser

Out of scope:
- novice-user UX

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers runtime-shape assertions for single-container, TLS, real CockroachDB, and scoped PostgreSQL role
- [ ] The E2E suite fails if it drifts into superuser or non-TLS shortcuts
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

