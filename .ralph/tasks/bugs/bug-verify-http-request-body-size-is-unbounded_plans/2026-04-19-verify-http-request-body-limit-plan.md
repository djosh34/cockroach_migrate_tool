# Plan: Bound Verify-Service Request Body Size At The Decode Boundary

## References

- Task: `.ralph/tasks/bugs/bug-verify-http-request-body-size-is-unbounded.md`
- Current verify-service request boundary:
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/http_test.go`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This bug is a request-boundary hardening fix, not a new feature.
  - The request shapes for `POST /jobs` and `POST /stop` are intentionally tiny.
  - The bug is that the shared JSON decoder reads from an unbounded `r.Body`.
- No backwards compatibility is required.
  - It is acceptable to reject oversized request bodies with an explicit client error.
  - Existing tests that rely on generic `400` decode failures should be narrowed to the new explicit oversized-body contract where appropriate.
- The task markdown plus this plan are the approval for the interface direction in this turn.
  - No extra config surface should be invented for one tiny, security-sensitive request boundary.
- Required validation lanes for execution remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` stays out of scope unless execution unexpectedly changes a story-end or e2e boundary.
- If the first RED slice proves the limit cannot be owned cleanly by one shared request-decoding boundary without handler-specific branching or string matching scattered across the code, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- `handlePostJobs` and `handlePostStop` both call the same helper:
  - `decodeJSONBody(r, destination)`
- `decodeJSONBody` is already the canonical JSON-input boundary for the HTTP write surface.
  - It disallows unknown fields.
  - It rejects trailing JSON documents.
  - It does not cap request size before decode work begins.
- That is the main boundary smell.
  - The service already has one shared request validation helper, but it stops short of owning the full HTTP body boundary.
  - Leaving body-size enforcement out of the helper would force route-specific request hardening and duplicate policy.
- The first bug report already names the correct public behavior:
  - oversized `POST /jobs` input should be explicitly rejected
  - the runner must not start

## Improve-Code-Boundaries Focus

- Primary smell: incomplete shared request decoder.
  - `decodeJSONBody` owns JSON-structure validation but not size validation.
  - Execution should deepen that helper so request-shape and request-size policy live in one place.
- Secondary smell: tiny-input routes without a single canonical limit.
  - `POST /jobs` and `POST /stop` should not each choose their own ad hoc byte cap in-line.
  - One canonical limit constant should describe the verify-service control-plane request budget.
- Preferred cleanup direction:
  - add a shared bounded-body helper or deepen `decodeJSONBody` directly
  - keep handlers focused on business behavior, not HTTP byte-budget mechanics
  - avoid introducing config, DTOs, or extra modules for a fixed small limit

## Public Contract After Execution

- `POST /jobs`
  - must reject bodies above the verify-service request-size limit with `413 Request Entity Too Large`
  - must not start a runner when the body is oversized
- `POST /stop`
  - must reject bodies above the same request-size limit with `413 Request Entity Too Large`
  - must not stop any job when the body is oversized
- Valid small JSON bodies must keep the current behavior:
  - unknown fields are still rejected
  - trailing documents are still rejected
  - empty/default `{}` for `POST /jobs` still works
- The exact byte cap should be one internal constant sized for the current tiny control-plane payloads.
  - It should be large enough for real requests with filters and job ids.
  - It should stay small enough that the service never does meaningful work on arbitrarily large bodies.

## Expected Code Shape

- `cockroachdb_molt/molt/verifyservice/service.go`
  - move full request-boundary enforcement behind one shared helper
  - translate oversized-body errors into explicit `413` responses
  - keep both write handlers using the same bounded decoder path
- `cockroachdb_molt/molt/verifyservice/http_test.go`
  - add integration-style request-boundary tests through the public HTTP API
  - prove oversized requests are rejected before job start or stop side effects happen
- Avoid:
  - per-handler `http.MaxBytesReader` calls duplicated in-line
  - stringly size checks spread across multiple handlers
  - adding a runtime config knob for a fixed tiny API body limit

## Type And Boundary Decisions

- Preferred boundary shape:
  - deepen `decodeJSONBody` so it enforces a single max body size for verify-service control-plane JSON input
- Acceptable implementation detail:
  - use `http.MaxBytesReader` at the shared helper boundary
  - introduce one sentinel error or typed error path for oversized bodies so handlers do not branch on fragile raw stdlib strings
- Do not add:
  - handler-local byte limits
  - a public config field for the limit
  - separate decoder helpers per route unless the first red slice proves the shared boundary is wrong

## TDD Execution Order

### Slice 1: Tracer Bullet For `POST /jobs`

- [x] RED: add one failing HTTP integration test proving oversized `POST /jobs` returns `413` and the runner never starts
- [x] GREEN: make the smallest change that enforces the shared request-size limit and maps the oversize condition to `413`
- [x] REFACTOR: keep the body-size policy inside the shared decode boundary instead of duplicating handler logic

### Slice 2: Verify The Bug Still Holds For `POST /stop`

- [x] RED: manually verify whether `POST /stop` is still unbounded after Slice 1; if it is, add one failing integration test proving oversized `POST /stop` returns `413` and does not stop the active job
- [x] GREEN: extend the shared decode boundary so `POST /stop` gets the same explicit limit without route-specific duplication
- [x] REFACTOR: collapse any branching or helper duplication introduced while covering both write endpoints

### Slice 3: Focused Package Validation

- [x] RED: run focused `verifyservice` tests again and let the next failure expose any remaining request-boundary assumption
- [x] GREEN: keep all verify-service request decoder behavior passing with the new explicit size limit
- [x] REFACTOR: do one final `improve-code-boundaries` pass so one canonical helper owns small-JSON control-plane input validation

### Slice 4: Repository Validation

- [x] Run `make check`
- [x] Run `make lint`
- [x] Run `make test`
- [x] `make test-long` is not required unless execution unexpectedly changes a story-end or e2e contract

## Expected Boundary Outcome

- Verify-service write routes get one coherent public-input boundary:
  - bounded size
  - strict JSON shape
  - explicit client error on oversize
- The code should get cleaner, not broader:
  - one shared request-size policy
  - handlers that only own route behavior
  - no config sprawl or duplicated byte-limit wiring

Plan path: `.ralph/tasks/bugs/bug-verify-http-request-body-size-is-unbounded_plans/2026-04-19-verify-http-request-body-limit-plan.md`

NOW EXECUTE
