## Task: Prove HTTP request inputs cannot cause command injection in verify execution <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete


**Goal:** Add a separate hardening task that proves request inputs such as table or schema filters cannot be transformed into command injection or equivalent unsafe process invocation when verify jobs are launched. The higher order goal is to isolate the highest-risk input-handling concern into its own explicit task instead of burying it inside the general HTTP server work.

In scope:
- threat-model the remaining live HTTP inputs, especially include/exclude table/schema filters, job-start parameters, live-status lookup, and stop control with optional `job_id`
- prove verify execution is launched without shell interpolation
- add explicit tests for hostile input values and argument-boundary handling
- verify database connection details still remain config-only and are never influenced by request payloads

Out of scope:
- broader multi-pass security review across the whole service
- general HTTP endpoint design outside injection resistance

Decisions already made:
- request payloads are untrusted
- connection details must come only from config, which already removes a major injection and leakage path
- only narrowly-scoped filters may be accepted through HTTP
- both include and exclude filters are allowed live inputs and must be covered by the injection hardening work
- this injection check must be a separate task from building the server
- concurrent start attempts should return conflict rather than changing execution state implicitly
- `/stop` accepts optional `job_id`, so both stop-all and stop-targeted flows must be covered by the hardening work
- job results include mismatch and failure-reason payloads over JSON, so those returned fields must not introduce unsafe rendering or command-construction behavior

</description>


<acceptance_criteria>
- [x] Red/green TDD covers hostile-input cases for every live HTTP field accepted by the job-start, job-status, and stop APIs, including include/exclude filters and optional `job_id`
- [x] Verify execution is proven to avoid shell interpolation and to preserve strict argument boundaries for allowed inputs
- [x] Tests fail if request payloads can alter connection settings or turn allowed filters into command-injection vectors
- [x] Tests cover targeted-stop `404` handling and verify that status/reason/mismatch JSON fields are derived safely from execution results
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [x] `make lint` — passes cleanly
- [x] If this task impacts ultra-long tests (or their selection): `make test-long` — not required; this task did not change the long-lane selection boundary
</acceptance_criteria>

<plan>.ralph/tasks/story-18-verify-http-image/06-task-prove-http-request-inputs-cannot-cause-command-injection-in-verify-execution_plans/2026-04-19-verify-http-injection-hardening-plan.md</plan>
