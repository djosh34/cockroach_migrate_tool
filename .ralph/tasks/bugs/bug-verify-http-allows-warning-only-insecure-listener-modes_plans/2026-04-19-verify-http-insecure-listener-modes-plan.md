# Plan: Reject Warning-Only Insecure Verify-Service Listener Modes

## References

- Task: `.ralph/tasks/bugs/bug-verify-http-allows-warning-only-insecure-listener-modes.md`
- Prior verify-service config task and plan:
  - `.ralph/tasks/story-18-verify-http-image/04-task-add-verify-service-config-for-source-destination-tls-and-mode-selection.md`
  - `.ralph/tasks/story-18-verify-http-image/04-task-add-verify-service-config-for-source-destination-tls-and-mode-selection_plans/2026-04-19-verify-service-config-plan.md`
- Current security boundary:
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - `cockroachdb_molt/molt/verifyservice/config_test.go`
  - `cockroachdb_molt/molt/verifyservice/testdata/*.yml`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This bug is a security-boundary correction, not a new feature.
  - The verify service is a remote control plane.
  - Deployment-time discipline is not an acceptable substitute for enforced transport and caller-auth policy.
- No backwards compatibility is required.
  - Configs using `listener.transport.mode: http` or `listener.tls.client_auth.mode: none` must become invalid.
  - Warning-only acceptance of insecure listener modes must be removed rather than preserved.
- The required repository validation lanes for this task remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` is not part of this task unless execution proves this bug changed the long-lane boundary.
- If the first red slice shows that the intended public contract is not actually “verify-service listener must be HTTPS with mTLS”, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- `Config.Validate()` currently accepts two insecure listener states:
  - `listener.transport.mode: http`
  - `listener.tls.client_auth.mode: none`
- `cmd/verifyservice/verifyservice.go` treats that accepted insecure state as valid and prints:
  - `warning: no extra built-in protection is being provided by the verify service`
- `runtime.go` still contains a real HTTP serving branch.
  - That means the security boundary is not enforced in config; runtime still knows how to serve insecurely.
- Existing fixtures and tests encode the wrong contract.
  - `valid-passwordless-client-cert.yml` uses `http` and `none`.
  - command tests assert that warning-only insecure startup is acceptable.

## Interface And Boundary Decisions

- Tighten the listener security contract to one allowed operating mode:
  - `listener.transport.mode` must be `https`
  - `listener.tls.client_auth.mode` must be `mtls`
- Move the security policy fully into validated config.
  - A valid `verifyservice.Config` must already guarantee secure listener transport and caller auth.
  - Commands and runtime must not carry a second “warn but still run insecurely” policy layer.
- Remove dead insecure-mode behavior instead of keeping it around behind warnings.
  - Delete the warning path derived from `DirectServiceAuthWarning()`.
  - Delete runtime support for plaintext HTTP serving.
  - Delete or rewrite fixtures whose only purpose is demonstrating insecure listener modes.
- Keep passwordless database client-certificate support.
  - The database TLS boundary is unrelated to this listener hardening bug and should stay supported.
  - Replace insecure listener settings in those fixtures with the required secure listener contract instead of dropping the fixture entirely.

## Improve-Code-Boundaries Focus

- Primary smell: validation outside config.
  - Security policy currently lives partly in config validation and partly in command warning output.
  - Execution should collapse that split so the validated config is the only place that decides whether a listener mode is allowed.
- Secondary smell: wrong-place security knowledge.
  - `runtime.go` should not know about an insecure HTTP branch if the product no longer permits it.
  - The runtime should serve only the validated secure mode.
- Tertiary smell: muddy fixture intent.
  - `valid-passwordless-client-cert.yml` currently mixes two unrelated concerns:
    - database client-certificate auth
    - insecure listener policy
  - Execution should separate those concerns so fixtures describe one contract each.

## Public Contract After Execution

- `molt verify-service validate-config --config <path>` must fail for:
  - any config with `listener.transport.mode: http`
  - any config with `listener.tls.client_auth.mode: none`
- `molt verify-service run --config <path>` must also fail on those configs because `LoadConfig()` fails first.
- A valid verify-service listener config must include:
  - `listener.transport.mode: https`
  - `listener.tls.cert_path`
  - `listener.tls.key_path`
  - `listener.tls.client_auth.mode: mtls`
  - `listener.tls.client_auth.client_ca_path`
- Passwordless database client-certificate configs remain valid only when the listener itself is secure.
- There is no “accepted but warned” insecure listener mode anymore.

## Files And Structure To Change

- [x] `cockroachdb_molt/molt/verifyservice/config.go`
  - make the validated listener contract require secure transport and client auth
  - remove warning-only security acceptance
- [x] `cockroachdb_molt/molt/verifyservice/runtime.go`
  - remove plaintext HTTP serving support that should now be unreachable
- [x] `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - remove warning printing that represents the old insecure contract
- [x] `cockroachdb_molt/molt/verifyservice/config_test.go`
  - add red coverage for rejecting insecure listener transport and insecure client-auth modes
- [x] `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - replace warning-based command assertions with failure assertions for insecure configs
- [x] `cockroachdb_molt/molt/verifyservice/testdata/*.yml`
  - add explicit invalid insecure-listener fixtures
  - rewrite any remaining “valid” fixture to use HTTPS plus mTLS when its real purpose is database TLS coverage

## TDD Execution Order

### Slice 1: Tracer Bullet For Rejected Plain HTTP

- [x] RED: add one failing command or config test proving a config with `listener.transport.mode: http` is rejected, even if the rest of the file is otherwise valid
- [x] GREEN: make the smallest config validation change that rejects plaintext listener transport
- [x] REFACTOR: keep the failure owned by config validation, not by command-specific checks

### Slice 2: Reject HTTPS Without Caller Authentication

- [x] RED: add one failing test proving `listener.transport.mode: https` plus `listener.tls.client_auth.mode: none` is rejected
- [x] GREEN: narrow the validated listener auth contract so insecure direct service auth is invalid
- [x] REFACTOR: remove `DirectServiceAuthWarning()` if it no longer represents any valid state

### Slice 3: Keep Passwordless DB Client Cert Support Without Insecure Listener Spillover

- [x] RED: update the existing passwordless database-client-cert test fixture so it remains focused on DB connectivity while now requiring a secure listener
- [x] GREEN: rewrite or replace the fixture and keep the database connection behavior green under the new listener contract
- [x] REFACTOR: make sure no fixture combines unrelated listener-policy and database-auth concerns

### Slice 4: Delete Dead Runtime Branches

- [x] RED: run the smallest focused Go test set that exposes any remaining runtime or command references to insecure listener modes
- [x] GREEN: remove the plaintext HTTP serving branch and any command warning output that became unreachable
- [x] REFACTOR: keep runtime limited to secure serving behavior implied by validated config

### Slice 5: Full Verify-Slice Validation

- [x] RED: run focused Go tests for `verifyservice` and `cmd/verifyservice`, fixing the next failing public behavior one slice at a time
- [x] GREEN: end with focused Go package tests plus the default `make test` verify-image contract coverage that exercises the secure runtime end to end
- [x] REFACTOR: do one final `improve-code-boundaries` pass and remove any leftover insecure-mode enum values, warning helpers, or dead branches if they no longer belong

### Slice 6: Repository Validation Lanes

- [x] Run `make check`
- [x] Run `make lint`
- [x] Run `make test`
- [x] `make test-long` was not required because the bug impact stayed inside the default `make test` lane

## Expected Boundary Outcome

- The listener security contract becomes self-enforcing instead of advisory.
- `verifyservice.Config` becomes a deeper module:
  - callers either get a secure validated config
  - or they get a loud error before runtime starts
- There is no second policy plane in CLI output or runtime branching.
- Insecure listener modes stop existing as “valid but discouraged” product behavior.

Plan path: `.ralph/tasks/bugs/bug-verify-http-allows-warning-only-insecure-listener-modes_plans/2026-04-19-verify-http-insecure-listener-modes-plan.md`

NOW EXECUTE
