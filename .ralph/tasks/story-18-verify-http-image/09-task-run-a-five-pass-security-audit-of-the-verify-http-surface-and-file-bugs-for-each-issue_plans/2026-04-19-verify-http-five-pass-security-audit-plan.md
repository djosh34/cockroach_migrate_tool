# Plan: Run A Five-Pass Security Audit Of The Verify HTTP Surface And File Bugs For Every Issue Found

## References

- Task:
  - `.ralph/tasks/story-18-verify-http-image/09-task-run-a-five-pass-security-audit-of-the-verify-http-surface-and-file-bugs-for-each-issue.md`
- Prerequisite verify HTTP work and plans:
  - `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs.md`
  - `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs_plans/2026-04-19-verify-http-job-api-plan.md`
  - `.ralph/tasks/story-18-verify-http-image/06-task-prove-http-request-inputs-cannot-cause-command-injection-in-verify-execution.md`
  - `.ralph/tasks/story-18-verify-http-image/06-task-prove-http-request-inputs-cannot-cause-command-injection-in-verify-execution_plans/2026-04-19-verify-http-injection-hardening-plan.md`
  - `.ralph/tasks/story-18-verify-http-image/08-task-expose-verify-job-progress-and-result-metrics.md`
- Current verify HTTP security surface:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/filter.go`
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
  - `cockroachdb_molt/molt/verifyservice/metrics.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/filter_test.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
- Skill:
  - `tdd`
- Skill:
  - `improve-code-boundaries`
- Skill:
  - `add-bug`

## Planning Assumptions

- This task is an audit task first.
  - It may include narrow hardening or guardrail-test additions discovered during the audit.
  - It must not turn into a broad verify-service redesign or attempt to fix every discovered vulnerability in one turn.
- The live public surface under review is:
  - `POST /jobs`
  - `GET /jobs/{job_id}`
  - `POST /stop`
  - `GET /metrics`
  - `verifyservice.Run(...)`
  - verify-service config loading, DB TLS connection-string construction, and listener TLS bootstrap
- Every confirmed security issue must become its own bug task under `.ralph/tasks/bugs/`.
  - No batching of unrelated findings into one bug.
  - No silent TODO comments instead of bug tasks.
- Because the acceptance criteria require TDD coverage, execution must still use Red-Green slices for narrow hardening work and for adding durable security guardrails where practical.
  - If an issue is too large to fix inside this audit task, document the evidence in the audit artifact and create a bug task instead of trying to land a failing test in the main branch.
- No errors may be swallowed or ignored during the audit.
  - If the audit discovers a swallowed/ignored error pattern, that is itself a security or correctness bug and must become an `add-bug` task.
- If the first execution slice proves that the audit artifact shape, pass boundaries, or intended hardening seam must change materially, switch this plan back to `TO BE VERIFIED` immediately and stop.

## Current State Summary

- `POST /jobs` decodes JSON directly with `json.NewDecoder(r.Body).Decode(&jobRequest)` and then compiles the typed request once through `JobRequest.Compile()`.
- The request payload currently influences only name-filter regex strings.
  - Connection strings are derived from config in `VerifyRunner.Run`.
  - Verify execution is in-process through `verify.Verify(...)`, not by shelling out.
- `GET /jobs/{job_id}` and `POST /stop` treat `job_id` as an opaque lookup key.
- Completed jobs remain in memory until process exit.
  - `Service.jobs` is an unbounded map.
  - `/metrics` iterates all remembered jobs and their stored progress.
- `/metrics` exposes job IDs, database names derived from configured URLs, per-table row counts, mismatch kinds, and error counts.
- The listener can run in plain HTTP or HTTPS mode.
  - mTLS is optional.
  - `Config.DirectServiceAuthWarning()` currently warns about missing built-in protection instead of enforcing secure transport/auth by default.
- `writeJSON(...)` panics on encode errors.
  - That is not automatically a security bug, but it is part of the hostile-client review surface and must be evaluated during the resource-exhaustion pass.

## Audit Artifact Contract

- Execution must create and maintain one artifact file:
  - `.ralph/tasks/story-18-verify-http-image/artifacts/verify-http-security-audit/2026-04-19-five-pass-audit.md`
- That artifact must contain five clearly labeled sections:
  - `Pass 1`
  - `Pass 2`
  - `Pass 3`
  - `Pass 4`
  - `Pass 5`
- Each pass section must record:
  - scope
  - files and tests reviewed
  - attack hypotheses checked
  - findings or explicit no-finding conclusion
  - bug task path(s) created, if any
  - any narrow code/test hardening completed during that pass

## Interface And Boundary Decisions

- Keep the verify HTTP API shape unchanged during the audit unless a confirmed bug requires a narrow hardening change.
  - Do not widen the request DTO.
  - Do not add alternate endpoints or secondary auth/config channels.
- Keep config-only ownership of:
  - database URLs
  - DB TLS file paths
  - listener bind address
  - listener transport mode
  - listener TLS and client-auth files
- If request hardening is needed, centralize it behind one request-decoding boundary instead of sprinkling ad hoc checks through each handler.
- If exposure tightening is needed, centralize it in one response/metrics snapshot boundary instead of scattering redaction rules across handlers and collectors.
- Keep bug-filing workflow outside production code.
  - Audit notes and bug task paths belong in Ralph task artifacts, not in service comments.

## Improve-Code-Boundaries Focus

- Primary smell: security policy is currently implicit and split across handlers, config validation, metrics collection, and the runner seam.
  - If the audit leads to hardening changes, flatten those checks into one decode boundary and one visibility/snapshot boundary.
- Secondary smell: `service.go` mixes HTTP decoding, lifecycle control, JSON writing, and job/result ownership.
  - Do not make this worse by adding pass-specific one-off guards in random branches.
  - Prefer small helpers with clear ownership if hardening is needed.
- Tertiary smell: metrics and JSON responses both render from live job state.
  - If a finding requires redaction or retention changes, keep the source of truth singular instead of creating duplicate “public” and “internal” job structs.

## Five Audit Passes

### Pass 1: Request Decoding And Untrusted Input Handling

- Audit focus:
  - malformed JSON handling
  - extra trailing JSON documents
  - unknown fields that look like connection/TLS/process overrides
  - regex validation and compilation behavior
  - empty versus malformed request bodies
  - request-size or parser-abuse risks
- Files:
  - `verifyservice/service.go`
  - `verifyservice/filter.go`
  - `verifyservice/http_test.go`
  - `verifyservice/filter_test.go`
- Likely TDD seam:
  - one request-decoding helper or one handler-level decode boundary, if hardening is needed
- Expected outcomes:
  - either prove the request boundary is already strict enough for the supported contract
  - or file one bug per weakness such as permissive decode behavior, parser abuse, or request amplification

### Pass 2: Process Launch, Config Isolation, And Non-Shell Execution Guarantees

- Audit focus:
  - verify that request JSON cannot influence shell commands, argv construction, environment, DB URLs, or TLS file paths
  - verify that execution remains in-process typed Go calls
  - verify that config-only connection-string derivation still holds
- Files:
  - `verifyservice/verify_runner.go`
  - `verifyservice/verify_runner_test.go`
  - `verifyservice/runtime.go`
  - `verifyservice/config.go`
  - any `verifyservice` package search for `os/exec`, `exec.Command`, `sh -c`, or string-built command paths
- Likely TDD seam:
  - narrow runner-seam tests extending the existing config-only connection-string guarantees
- Expected outcomes:
  - either strengthen durable tests around non-shell/config-only behavior
  - or create separate bugs for any request-to-runtime leakage or process-launch risk

### Pass 3: Job Lifecycle, Cancellation, And Resource Exhaustion

- Audit focus:
  - single-active-job enforcement under hostile input
  - cancellation and stop semantics
  - unbounded job retention in memory
  - growth of status/mismatch/error accumulation
  - `/metrics` collection cost scaling with historical jobs
  - panic or crash behavior on hostile clients or write failures
- Files:
  - `verifyservice/service.go`
  - `verifyservice/metrics.go`
  - job progress/result helpers reached from those files
  - `verifyservice/http_test.go`
- Likely TDD seam:
  - one focused service/handler test for the first resource-boundary issue chosen for narrow hardening, if that hardening is in scope
- Expected outcomes:
  - document and file bugs for any DoS or unbounded-retention issues
  - keep any in-scope hardening minimal and boundary-driven rather than bolting on counters everywhere

### Pass 4: Result Rendering, Metrics Exposure, And Information Disclosure

- Audit focus:
  - what `GET /jobs/{job_id}` reveals to any caller
  - what `/metrics` reveals to any caller
  - leakage of database names, table names, mismatch contents, and failure reasons
  - whether hostile-looking result strings stay inert typed JSON rather than being reinterpreted
  - whether observability data exposes more than the supported product contract should expose
- Files:
  - `verifyservice/service.go`
  - `verifyservice/metrics.go`
  - `verifyservice/http_test.go`
- Likely TDD seam:
  - existing typed JSON/result tests and metrics tests, extended only where a narrow hardening change is actually made
- Expected outcomes:
  - preserve typed rendering guarantees
  - file one bug per confirmed disclosure issue or overly broad observability surface

### Pass 5: TLS Material Handling, Listener Policy, And Runtime Bootstrap

- Audit focus:
  - HTTPS versus HTTP policy
  - optional versus enforced mTLS
  - TLS CA loading and failure behavior
  - cert/key file handling and validation paths
  - whether runtime defaults are defensible for a remotely triggered verify surface
  - whether warnings are standing in for enforcement where security policy should be explicit
- Files:
  - `verifyservice/config.go`
  - `verifyservice/runtime.go`
  - command entrypoints that load config and start the server
- Likely TDD seam:
  - config/runtime tests for the first policy-tightening change that fits this task, if any
- Expected outcomes:
  - either document why the current transport/auth policy is acceptable for the supported contract
  - or create bugs for insecure defaults, missing enforcement, or unsafe TLS bootstrap assumptions

## TDD Execution Strategy

### Slice 1: Tracer Bullet On The First Missing Guardrail

- [x] RED: pick the first concrete security-property gap that should be fixed inside this task rather than deferred to a bug
  - likely candidates are strict JSON decoding or a similarly narrow input-boundary guardrail
- [x] GREEN: implement the smallest boundary-level hardening needed to make that one behavior pass
- [x] REFACTOR: keep the hardening at one decode or runner boundary, not spread across unrelated handlers

### Slice 2: One Security Property At A Time

- [x] RED: add one failing test for the next in-scope hardening behavior
- [x] GREEN: implement only enough to pass that test
- [x] REFACTOR: flatten any new conditionals behind existing typed boundaries

### Slice 3: For Confirmed Bugs That Are Out Of Scope To Fix Here

- [x] Record the evidence in the five-pass audit artifact
- [x] Create one bug file via `add-bug` format under `.ralph/tasks/bugs/bug-*.md`
- [x] Note the exact affected files, pass number, and expected public-contract test that the bug task should add
- [x] Do not leave the repository with intentionally failing tests

## Bug Filing Workflow

- Each confirmed issue gets its own bug file:
  - example slug shape: `.ralph/tasks/bugs/bug-verify-http-unknown-fields-accepted.md`
- Each bug description must include:
  - what is broken
  - how the audit detected it
  - why it is security-sensitive
  - the audit pass number
  - affected files or boundaries
  - the first Red test the bug task should add
- If multiple issues are discovered in one pass, create multiple bug files.
  - One bug per issue.
  - Do not hide multiple findings under one generic “security audit fixes” bug.

## Boundary Review Checklist For Execution

- [x] request decoding policy is explicit and centralized if any hardening is needed
- [x] request payload cannot influence DB URLs, TLS material paths, listener settings, or process launch
- [x] verify execution remains typed in-process Go calls with no shell boundary
- [x] hostile `job_id` and hostile result strings stay inert data
- [x] job retention and observability surfaces have been reviewed for resource exhaustion and disclosure risk
- [x] TLS/bootstrap policy has been reviewed for remotely exposed service safety
- [x] every confirmed issue has a separate bug task path recorded in the audit artifact
- [x] final `improve-code-boundaries` pass removes any audit-driven boundary mud rather than adding more

## Final Verification For The Execution Turn

- [x] focused Go tests for the touched `verifyservice` package
- [x] five-pass audit artifact completed with explicit findings or no-finding conclusions
- [x] every confirmed issue has a separate `add-bug` task file
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [ ] `make test-long`
  - not required unless execution changes the long-lane boundary or the task explicitly expands into a story-end gate
- [x] update task acceptance checkboxes and set `<passes>true</passes>` only after the required lanes pass

Plan path: `.ralph/tasks/story-18-verify-http-image/09-task-run-a-five-pass-security-audit-of-the-verify-http-surface-and-file-bugs-for-each-issue_plans/2026-04-19-verify-http-five-pass-security-audit-plan.md`

NOW EXECUTE
