# Plan: Deepen Verify-Service HTTPS Certificate Bootstrap

## References

- Task: `.ralph/tasks/bugs/bug-verify-http-https-runtime-does-not-load-server-certificate.md`
- Current verify-service listener code:
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/config_test.go`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- This bug is a TLS bootstrap boundary correction, not a new feature.
  - The listener already requires HTTPS plus mTLS at config-validation time.
  - The missing part is that server certificate ownership is still split between `Run(...)` and `ListenerTLSConfig.ServerTLSConfig()`.
- The task description is slightly ahead of the current code.
  - Today `Run(...)` still calls `server.ListenAndServeTLS(cfg.Listener.TLS.CertPath, cfg.Listener.TLS.KeyPath)`.
  - The intended end state is the standard-library empty-filename contract: preload the certificate into `server.TLSConfig`, then call `ListenAndServeTLS("", "")`.
- No backwards compatibility is required.
  - It is acceptable to change the internal bootstrap shape so runtime stops passing file paths around after config validation.
- Required validation lanes for this task remain:
  - `make check`
  - `make lint`
  - `make test`
- `make test-long` is out of scope unless execution unexpectedly changes a story-end or e2e boundary.
- If the first red slice proves the secure listener bootstrap cannot be expressed cleanly through `ListenerTLSConfig.ServerTLSConfig()`, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- `ListenerConfig.validate()` already requires:
  - `listener.transport.mode: https`
  - `listener.tls.cert_path`
  - `listener.tls.key_path`
  - `listener.tls.client_auth.mode: mtls`
  - `listener.tls.client_auth.client_ca_path`
- `ListenerTLSConfig.ServerTLSConfig()` currently does only part of the listener TLS job.
  - It creates a base `tls.Config`.
  - It loads the client CA pool for mTLS.
  - It does not load the configured server certificate and key into `tls.Config.Certificates`.
- `Run(...)` compensates for that incomplete boundary by still owning certificate-file wiring.
  - It builds `server.TLSConfig` through `ServerTLSConfig()`.
  - It then separately passes `cfg.Listener.TLS.CertPath` and `cfg.Listener.TLS.KeyPath` into `ListenAndServeTLS(...)`.
- That split ownership is the main boundary smell.
  - Listener TLS material is described by `ListenerTLSConfig`, but runtime still knows the file-level details.
  - The deeper module should be `ListenerTLSConfig`: one public method that returns the complete TLS server config or a loud error.
- There is currently no test proving the complete secure bootstrap contract.
  - Existing tests validate config shape.
  - They do not prove that a valid listener TLS config actually preloads the server certificate into `tls.Config`.

## Improve-Code-Boundaries Focus

- Primary smell: incomplete TLS bootstrap module.
  - `ListenerTLSConfig` is the natural owner of listener certificate loading.
  - Execution should make it own both server certificate material and client-auth CA material.
- Secondary smell: runtime knows too much.
  - `Run(...)` should only assemble the server and serve it.
  - It should not keep threading cert/key file paths once a validated `ListenerTLSConfig` exists.
- Preferred cleanup direction:
  - move PEM file loading behind `ServerTLSConfig()`
  - remove duplicate TLS wiring from `Run(...)`
  - do not introduce a second helper layer unless it materially deepens the boundary

## Public Contract After Execution

- `ListenerTLSConfig.ServerTLSConfig()` must return a usable server-side `*tls.Config` that:
  - has `MinVersion` set as today
  - has one loaded server certificate in `Certificates`
  - enforces mTLS with the configured client CA pool
- If `cert_path`, `key_path`, or `client_ca_path` cannot be read or parsed, `ServerTLSConfig()` must fail loudly.
  - No silent fallback.
  - No partially initialized HTTPS listener.
- `Run(...)` must rely on the preloaded `server.TLSConfig` and use `server.ListenAndServeTLS("", "")`.
  - That keeps the runtime aligned with the standard-library contract the task calls out.
  - It also makes the TLS bootstrap boundary coherent: one config boundary prepares TLS, one runtime boundary serves it.

## Files And Structure To Change

- `cockroachdb_molt/molt/verifyservice/runtime.go`
  - deepen `ListenerTLSConfig.ServerTLSConfig()` so it loads server certificate material
  - simplify `Run(...)` to use the preloaded TLS config only
- `cockroachdb_molt/molt/verifyservice/config_test.go` or a new focused `runtime_test.go`
  - add behavior-level TLS bootstrap coverage
- Optional:
  - introduce one private helper inside `verifyservice` only if it meaningfully reduces duplicate PEM-loading logic
  - avoid creating extra DTOs or layers for a small bootstrap concern

## Test Strategy

- Prefer behavior tests through the public `ListenerTLSConfig.ServerTLSConfig()` boundary rather than implementation-only seams.
- Generate temporary certificate, key, and client-CA PEM material inside the test when possible.
  - That keeps the test self-contained.
  - That avoids checking in extra static cert fixtures unless execution proves they make the test materially clearer.
- Only add a second red test after verifying the first fix still leaves a gap.
  - Follow the required vertical TDD rhythm: one failing test, one green step, repeat.

## TDD Execution Order

### Slice 1: Tracer Bullet For Complete TLS Bootstrap

- [x] RED: add one failing test proving `ListenerTLSConfig.ServerTLSConfig()` loads a valid server certificate/key pair into `tls.Config.Certificates` while preserving the configured mTLS client CA behavior
- [x] GREEN: make the smallest code change that loads the server key pair inside `ServerTLSConfig()`
- [x] REFACTOR: keep certificate loading local to the listener TLS boundary instead of spreading it across runtime and config

### Slice 2: Runtime Uses The Complete Boundary

- [x] RED: run the smallest focused verify-service test step that reveals whether runtime still depends on passing cert/key filenames directly
- [x] GREEN: change `Run(...)` to call `server.ListenAndServeTLS("", "")` once `server.TLSConfig` is fully prepared
- [x] REFACTOR: remove any remaining duplicate references to listener cert/key file paths from the runtime path

### Slice 3: Focused Package Validation

- [x] RED: run focused verify-service tests again and fix only the next failing public behavior
- [x] GREEN: keep all verify-service package tests passing without reintroducing split TLS ownership
- [x] REFACTOR: do one final `improve-code-boundaries` pass and collapse any leftover helper or branch that exists only because TLS bootstrap used to be split

### Slice 4: Repository Validation Lanes

- [x] Run `make check`
- [x] Run `make lint`
- [x] Run `make test`
- [x] `make test-long` is not required unless execution unexpectedly changes a story-end or e2e contract

## Expected Boundary Outcome

- `ListenerTLSConfig` becomes the single deep module for verify-service listener TLS bootstrap.
- Runtime stops knowing PEM file details after it receives a validated config.
- HTTPS startup is covered by a real red-green test instead of relying on implicit stdlib behavior.
- The code should get smaller and cleaner:
  - one TLS bootstrap boundary
  - one serving boundary
  - no duplicate certificate-loading responsibilities

Plan path: `.ralph/tasks/bugs/bug-verify-http-https-runtime-does-not-load-server-certificate_plans/2026-04-19-verify-http-server-certificate-bootstrap-plan.md`

NOW EXECUTE
