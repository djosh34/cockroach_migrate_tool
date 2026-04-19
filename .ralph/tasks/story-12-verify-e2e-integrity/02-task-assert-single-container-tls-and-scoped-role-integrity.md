## Task: Assert single-container, TLS, and scoped-role integrity in end-to-end tests <status>completed</status> <passes>true</passes>

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
 - [x] Red/green TDD covers runtime-shape assertions for single-container, TLS, real CockroachDB, and scoped PostgreSQL role
 - [x] The E2E suite fails if it drifts into superuser or non-TLS shortcuts
 - [x] `make check` — passes cleanly
 - [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
 - [x] `make lint` — passes cleanly
 - [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-12-verify-e2e-integrity/02-task-assert-single-container-tls-and-scoped-role-integrity_plans/2026-04-19-single-container-tls-scoped-role-integrity-plan.md</plan>
