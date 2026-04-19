## Task: Build an ultra-scoped HTTP job API for single active verify runs using config-defined connections only <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Expose an ultra-simple HTTP service around verify with single-active-job semantics, explicit job identifiers, stop control, and job-status retrieval while keeping the HTTP surface intentionally dumb and tightly scoped. The higher order goal is to make verification remotely triggerable over HTTP without letting callers influence database connection details or expand the API into a second configuration plane.

In scope:
- `POST` endpoint to start a verify action and return a job identifier
- `GET` endpoint to fetch live job status and final job result
- `POST` stop endpoint that stops all active verify work when no `job_id` is given, or stops only the referenced job when a `job_id` is provided
- storage and retrieval of full verify JSON output including status, failure reason, mismatches, and errors
- one POST request creates one verify job
- only one verify job may run at a time
- no persistence; jobs and results may be lost on restart
- all database connection details and verify mode come from static config only
- allow only tightly-scoped request inputs such as include/exclude table/schema filters if explicitly supported
- support operation behind an authenticated proxy and direct HTTPS mTLS service mode

Out of scope:
- metrics endpoint
- cross-suite test enforcement
- the dedicated command-injection proof task

Decisions already made:
- the verify image only verifies via HTTP
- connection details must not be changeable via HTTP
- the HTTP API should stay ultra scoped: create verify job, read verify result, and stop active jobs
- jobs are in-memory only for now and are lost on restart
- only one verify job should run at a time
- each start must return a `job_id`, and `GET` must let the caller prove it is reading the result for that same started job
- a new start request while a job is already running should return conflict rather than replacing the active job
- `GET` should expose live status while the job is still running
- `GET` for a known `job_id` should remain readable as often as the caller wants until process restart, always returning the newest current or final result for that job
- `/stop` without a `job_id` should stop all active verify jobs, and `/stop` with a `job_id` should stop only that job
- both include and exclude table/schema filters may be allowed as scoped live inputs
- direct service authentication is recommended but not mandatory; when disabled, the lack of additional built-in protection must be explicit
- live/final job states should be minimal and explicit: `running`, `succeeded`, `failed`, `stopped`
- API responses should be JSON and include failure reason plus mismatch output when available from the underlying verify run
- targeted stop against an unknown or non-active `job_id` should return `404`

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers single-active-job behavior, `409 Conflict` on concurrent start attempts, job creation, live job lookup, stop behavior, in-memory job lifecycle, restart-loss behavior, and optional scoped filter inputs
- [ ] `POST` returns a stable job identifier, `GET` returns JSON live status or final verify result for that same job including mismatches and failure reason, the service does not allow ambiguous job/result mapping, and repeated `GET` calls keep returning the newest result until process restart
- [ ] The service returns only the explicit states `running`, `succeeded`, `failed`, and `stopped`
- [ ] The service returns conflict when a concurrent start is attempted, `POST /stop` without a `job_id` stops all active verify work, `POST /stop` with a `job_id` stops only that job, and unknown/non-active targeted stops return `404`
- [ ] The HTTP API cannot change database connection details, TLS material, or verify mode, and remains limited to create-job/read-result behavior
- [ ] If direct service authentication is disabled, the service and docs state explicitly that no extra built-in protection is being provided
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
