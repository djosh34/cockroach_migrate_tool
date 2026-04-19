# Plan: Prove HTTP Request Inputs Cannot Cause Command Injection In Verify Execution

## References

- Task:
  - `.ralph/tasks/story-18-verify-http-image/06-task-prove-http-request-inputs-cannot-cause-command-injection-in-verify-execution.md`
- Prior HTTP runtime task and plan:
  - `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs.md`
  - `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs_plans/2026-04-19-verify-http-job-api-plan.md`
- Current HTTP request and runtime boundary:
  - `cockroachdb_molt/molt/verifyservice/filter.go`
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner.go`
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/verifyservice/config.go`
- Existing legacy filter helpers that must stay outside the HTTP request path:
  - `cockroachdb_molt/molt/cmd/internal/cmdutil/name_filter.go`
  - `cockroachdb_molt/molt/utils/obj_filter.go`
- Skill:
  - `tdd`
- Skill:
  - `improve-code-boundaries`

## Planning Assumptions

- Task 05 already established the HTTP API shape.
  - This task should harden and prove that shape, not invent new endpoints.
- The request payload is untrusted.
  - Hostile strings in include/exclude filters, `job_id`, and result text must remain inert data.
- Database URLs and TLS material remain config-only.
  - No HTTP payload may influence `verify.source`, `verify.destination`, or listener TLS/auth settings.
- The current runtime already executes verify in-process.
  - `verifyservice.VerifyRunner.Run` calls `dbconn.Connect(...)` and `verify.Verify(...)`.
  - No `os/exec`, `exec.Command`, `sh -c`, or similar shell/process construction exists in the current verify-service path.
- If the first RED slice proves that the current runtime must shell out or must widen the request shape to stay testable, this plan is wrong and must be switched back to `TO BE VERIFIED` immediately.

## Current State Summary

- `POST /jobs` decodes `JobRequest`, validates regex strings, and starts a job.
- `GET /jobs/{job_id}` and `POST /stop` treat `job_id` as an opaque string lookup key.
- `VerifyRunner.Run` builds source and destination connection strings only from `Config`.
- `VerifyRunner.Run` currently mixes several responsibilities in one function:
  - config-only connection string derivation
  - connection establishment
  - verify invocation
  - filter translation from HTTP request into `utils.FilterConfig`
- `JobRequest.Validate()` and `JobRequest.FilterConfig()` both traverse the raw filter strings.
  - That leaves a stringly boundary in place longer than necessary.
- Result JSON is already rendered from typed verify reports, not parsed logs.
  - This is the right direction and should be preserved.

## Interface And Boundary Decisions

- Keep the public HTTP surface unchanged:
  - `POST /jobs`
  - `GET /jobs/{job_id}`
  - `POST /stop`
- Keep the current request DTO shape unchanged:

```json
{
  "filters": {
    "include": {
      "schema": "^public$",
      "table": "^(accounts|orders)$"
    },
    "exclude": {
      "schema": "^audit$",
      "table": "^tmp_"
    }
  }
}
```

- Keep `job_id` as opaque inert data.
  - No normalization, splitting, interpolation, or command-style reuse.
- Do not add any request fields for URLs, TLS files, ports, commands, environment variables, or flags.
- Harden the internal request boundary by replacing ad hoc string traversal with one typed request-scoped filter value.
  - The service should compile/validate request filters once.
  - The runner should consume a typed filter spec or already-derived DB filter config, not raw request JSON strings plus duplicate validation.
- Introduce the smallest test seam necessary around verify execution.
  - The goal is not more abstraction.
  - The goal is one narrow typed seam that lets tests prove:
    - connection settings come only from config
    - request data only affects the DB filter input
    - execution uses typed Go calls, not shell or argv string construction

## Improve-Code-Boundaries Focus

- Primary smell: mixed responsibilities in `verify_runner.go`.
  - One function currently owns config-only connection materialization, connection dialing, request filter translation, and verify execution.
  - Execution should flatten this into a smaller typed boundary with explicit ownership.
- Secondary smell: stringly request filter boundary in `filter.go`.
  - `Validate()` and `FilterConfig()` duplicate traversal of raw strings.
  - Compile and freeze the request-scoped filter value once, then pass that value down.
- Explicit non-goal:
  - Do not create a helper jungle or a second configuration layer just to make tests pass.
  - Keep the refactor deep and small: fewer raw strings crossing boundaries, not more wrappers.

## Proposed Code Shape

- `cockroachdb_molt/molt/verifyservice/filter.go`
  - keep JSON DTO types for the HTTP boundary
  - add one request-scoped typed filter value or conversion entrypoint
  - ensure regex compilation happens once for the accepted live inputs
- `cockroachdb_molt/molt/verifyservice/verify_runner.go`
  - extract the config-only execution inputs behind a narrow typed seam
  - inject only the minimum dependencies needed to record connect strings and verify invocation in tests
- `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
  - add focused proof tests for config-only connection materialization and request-only filter influence
- `cockroachdb_molt/molt/verifyservice/http_test.go`
  - extend hostile-input coverage for job start, job lookup, stop, and result rendering
- `cockroachdb_molt/molt/verifyservice/service.go`
  - adjust only if the typed filter boundary or safe response rendering needs minor plumbing changes

## TDD Slices

### Slice 1: Config-Only Execution Inputs

- [x] RED: add one failing `verify_runner` test that uses hostile request filter values plus deliberately recognizable config URLs/TLS paths and proves the runner derives source and destination connection strings only from `Config`
- [x] GREEN: add the smallest runner seam so the test can observe the config-derived connection strings and the typed verify invocation inputs without opening real DB connections
- [x] REFACTOR: keep the seam typed and narrow; do not introduce command-string builders, option bags, or mutable globals

### Slice 2: Request-Scoped Filter Boundary

- [x] RED: add one failing test that proves hostile but valid include/exclude regex inputs are accepted as literal filter values, while invalid regex still fails fast before execution
- [x] GREEN: replace duplicate raw-string traversal with one request-scoped typed filter conversion/validation path
- [x] REFACTOR: keep HTTP DTO decoding at the edge and keep `utils.FilterConfig` derivation in one place only

### Slice 3: Request Payload Cannot Alter Connection Settings

- [x] RED: add one failing HTTP-to-runner test that submits extra hostile JSON fields attempting to resemble connection or TLS overrides and proves execution still uses config-only connection settings
- [x] GREEN: ensure the request boundary ignores unknown fields from a security perspective and that no request field is ever threaded into connection-string derivation
- [x] REFACTOR: if any code path re-reads config-like values from request DTOs or handler-local maps, delete that path instead of guarding it

### Slice 4: Hostile `job_id` Values Stay Inert

- [x] RED: add one failing handler test that sends hostile `job_id` strings through `GET /jobs/{job_id}` and targeted `POST /stop`, proving they behave only as opaque lookup keys and return the expected `404` or stop behavior
- [x] GREEN: keep `job_id` handling as direct key lookup with no interpolation, parsing, or shell-style splitting
- [x] REFACTOR: if handler code starts branching on `job_id` shape beyond empty-versus-present semantics, flatten it back down

### Slice 5: Result Strings Render Safely

- [x] RED: add one failing HTTP test that records mismatch info and failure reason containing shell metacharacters, quotes, semicolons, or subshell-looking text and proves the JSON response returns those values literally
- [x] GREEN: keep result rendering on the existing typed JSON path only
- [x] REFACTOR: if any rendering path starts concatenating strings into pseudo-commands, delete it and keep typed JSON ownership in `verifyservice`

### Slice 6: Focused Go Package Validation

- [x] RED: run focused Go tests for `verifyservice` and fix the first failure only
- [x] GREEN: end with package-level Go tests green for the touched verify-service slice
- [x] REFACTOR: if the tests reveal overlap between HTTP DTO validation and runner execution setup, remove the overlap before moving on

### Slice 7: Repository Validation Lanes

- [x] RED: run `make check`, `make lint`, and `make test`, fixing only the first failing lane at a time
- [x] GREEN: continue until all required repository gates pass cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass focused on `verify_runner.go` and `filter.go` so execution ends with fewer raw-string boundaries than it started with

## TDD Guardrails For Execution

- One failing test at a time.
- No horizontal slice of "all security tests first, all implementation second".
- Prefer behavior tests through the HTTP surface or a narrow typed runner seam.
- Do not add mocks for private collaborators when a small typed seam will do.
- Do not shell out in tests or production code.
- Do not widen the request contract "for flexibility".
- Do not add compatibility shims for legacy CLI filter globals.
- Do not swallow any validation or runtime errors.
  - Invalid regex stays an explicit request failure.
  - Runtime failure stays explicit `failure_reason` plus `result.errors`.

## Boundary Review Checklist

- [x] Request filters are compiled/validated once and then carried as a typed request-scoped value
- [x] `verify_runner` uses config-only connection inputs and request-only filter inputs
- [x] No request payload path can influence database URLs, TLS files, or listener settings
- [x] No verify-service runtime path constructs shell commands or interpolated argv strings
- [x] `job_id` remains opaque inert data for lookup and stop targeting
- [x] Result JSON is rendered from typed values only, including hostile-looking mismatch and failure strings
- [x] No mutable legacy CLI filter global is reintroduced into the HTTP path

## Final Verification For The Execution Turn

- [x] focused Go tests for `verifyservice`
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] `make test-long`
  Not required unless execution changes the long-lane selection boundary or the story explicitly demands it
- [x] final `improve-code-boundaries` pass after the required lanes are green
- [x] update the task acceptance checkboxes and set `<passes>true</passes>` only after the required lanes pass

Plan path: `.ralph/tasks/story-18-verify-http-image/06-task-prove-http-request-inputs-cannot-cause-command-injection-in-verify-execution_plans/2026-04-19-verify-http-injection-hardening-plan.md`

NOW EXECUTE
