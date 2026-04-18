## Task: Build the HTTPS webhook server and table-routing runtime <status>done</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Build the destination webhook server with real TLS and config-driven routing into the correct destination database and helper shadow table set. The higher order goal is to create the real ingress path used by production and by the no-cheating E2E tests.

In scope:
- HTTPS webhook server
- health endpoint if needed
- config-driven stream/database routing
- payload shape handling for row batches and resolved messages
- one binary in one container

Out of scope:
- full persistence logic
- reconcile behavior

This task must use an established HTTP/TLS library, not a hand-rolled server.

</description>


<acceptance_criteria>
- [x] Red/green TDD covers HTTPS startup, routing, and payload shape dispatch
- [x] The webhook path runs with real TLS
- [x] One destination binary in one container exposes the webhook endpoint for all configured streams
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>

<plan>.ralph/tasks/story-06-destination-ingest/01-task-build-https-webhook-server-and-routing_plans/2026-04-18-https-webhook-server-and-routing-plan.md</plan>
