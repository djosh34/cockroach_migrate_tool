# Plan: Add Verify-Service Config For Source, Destination, TLS, And Explicit Verify Modes

## References

- Task: `.ralph/tasks/story-18-verify-http-image/04-task-add-verify-service-config-for-source-destination-tls-and-mode-selection.md`
- Prior verify source-boundary task and plan:
  - `.ralph/tasks/story-18-verify-http-image/01-task-prune-the-codebase-down-to-a-verify-only-source-slice-and-prove-removal.md`
  - `.ralph/tasks/story-18-verify-http-image/01-task-prune-the-codebase-down-to-a-verify-only-source-slice-and-prove-removal_plans/2026-04-19-verify-source-slice-prune-plan.md`
- Prior verify image task and plan:
  - `.ralph/tasks/story-18-verify-http-image/03-task-build-a-scratch-verify-image-from-the-pruned-verify-source.md`
  - `.ralph/tasks/story-18-verify-http-image/03-task-build-a-scratch-verify-image-from-the-pruned-verify-source_plans/2026-04-19-verify-scratch-image-plan.md`
- Follow-up task:
  - `.ralph/tasks/story-18-verify-http-image/05-task-build-an-ultra-scoped-http-job-api-for-single-active-verify-runs.md`
- Current verify command and wrong-boundary connection loading:
  - `cockroachdb_molt/molt/cmd/root.go`
  - `cockroachdb_molt/molt/cmd/verify/verify.go`
  - `cockroachdb_molt/molt/cmd/internal/cmdutil/dbconn.go`
  - `cockroachdb_molt/molt/dbconn/config.go`
  - `cockroachdb_molt/molt/dbconn/dbconn.go`
- Existing TLS and verify-mode behavior inside the pruned Go slice:
  - `cockroachdb_molt/molt/mysqlurl/parse.go`
  - `cockroachdb_molt/molt/dbconn/mysql.go`
- Existing runner config boundary that task 04 must not reopen:
  - `crates/runner/src/config/mod.rs`
  - `crates/runner/src/config/parser.rs`
  - `crates/runner/tests/config_contract.rs`
- Skill: `tdd`
- Skill: `improve-code-boundaries`

## Planning Assumptions

- This is a config-contract task, not the HTTP job API task.
  - Task 04 should establish the dedicated verify-service config surface and validation boundary that task 05 consumes.
  - It must not absorb job lifecycle, request routing, stop semantics, or metrics exposure from task 05 and task 08.
- The verify service config must live in the Go verify slice, not in the Rust runner config.
  - Reintroducing a `verify:` block under the runner YAML would be the wrong boundary and would regress the existing Rust contract that rejects it.
- Repository validation lanes for this task remain:
  - `make check`
  - `make lint`
  - `make test`
- Those lanes do not execute Go tests today.
  - Execution must therefore run explicit local Go validation for the verify slice in addition to the required repo lanes.
- The service must be config-only for connection behavior.
  - No service CLI flags for source URL, target URL, TLS files, or verify mode.
  - No future HTTP request fields for connection URLs, TLS material, or verify-mode overrides.
- If the first RED slice proves that the proposed public config shape is wrong, or that the service needs a materially different top-level command boundary than the one planned here, this plan must stay `TO BE VERIFIED` and execution must stop immediately.

## Current State Summary

- The current retained Go verify surface is still CLI-flag driven.
  - `molt verify` registers shared `--source` and `--target` flags.
  - `cmdutil.LoadDBConns` connects directly from those two raw strings.
- `dbconn.Config` is under-reduced.
  - It is only `{ Source string, Target string }`.
  - It has no explicit TLS mode, no certificate-material pairing, and no room for a service listener config.
- The verify slice has no dedicated config package, no config file parser, and no verify-service command namespace yet.
- Database TLS handling currently lives inside URL/DSN parsing and driver-specific helpers.
  - That is acceptable for legacy CLI input, but it is the wrong direct boundary for a service config that must make TLS intent explicit and auditable.
- The runner config in Rust already rejects legacy verify sections.
  - That is a useful guardrail: task 04 should create a parallel verify-service config contract in Go, not tunnel this work back into the runner.
- Task 05 already depends on task 04's boundary.
  - It needs one static verified config surface for source connection, destination connection, DB TLS behavior, and listener protection mode before the HTTP API exists.

## Interface And Boundary Decisions

- Introduce a dedicated public command namespace for the service:
  - `molt verify-service validate-config --config <path>`
- Keep the future runtime command in the same namespace:
  - task 05 should add `molt verify-service run --config <path>` rather than inventing a second config entrypoint.
- Keep the config owned by a verify-service package inside the Go slice.
  - The command package should stay a thin adapter.
  - Parsing and validation should live in the verify-service config package, not in `cmdutil` and not in `main`.
- Use one canonical validated connection shape for both `source` and `destination`.
  - Do not create `SourceConnectionConfig` and `DestinationConnectionConfig` with duplicate fields.
  - If a field applies to both sides, it belongs in one shared `DatabaseConfig` or `DatabaseConnectionConfig`.
- Keep config validation inside the config package only.
  - No later `ensureTLS`, `ensureListener`, or `ensureClientAuth` helpers in runtime code.
  - The validated config must already guarantee those invariants.
- Make TLS verification mode explicit per database connection.
  - Allowed values are only `verify-full` and `verify-ca`.
  - Missing mode is invalid.
  - Any value outside those two is invalid.
- Keep listener transport/auth explicit and small.
  - One transport mode enum: `http` or `https`
  - One direct-auth mode enum: `none` or `mtls`
  - `none` is allowed but must be surfaced explicitly as "no extra built-in protection"
  - `mtls` is allowed only with HTTPS and a client CA path
- Keep DB URLs config-only for now.
  - The config may carry `url` plus explicit TLS settings because the existing verify slice still connects through DSN/URL-based driver entrypoints.
  - The service command and later HTTP API must not accept URL overrides.
- Keep URL-to-driver-connection rendering in one place.
  - The validated DB config should expose one method that materializes the final connection string for `dbconn.Connect`.
  - Do not duplicate source and destination TLS-to-URL mutation logic.

## Improve-Code-Boundaries Focus

- Primary smell: wrong-place config growth.
  - `cmd/internal/cmdutil/dbconn.go` is the current CLI-flag boundary.
  - Task 04 must not grow service config into that shared flag package.
  - The service config belongs in a dedicated verify-service package.
- Secondary smell: config not reduced.
  - A raw YAML struct plus separate runtime-normalization helpers would be muddy.
  - Parsing must yield one validated config type with resolved auth mode, validated cert/key pairs, and ready-to-render DB connection specs.
- Tertiary smell: validation outside config.
  - Listener TLS/material validation, client-auth validation, and DB TLS pairing rules must all happen during config loading.
  - The runtime should never re-check whether the config is well-formed.
- Quaternary smell: duplicated source/destination connection shapes.
  - Keep one shared DB connection type and one render path.
  - If `dbconn.Config` becomes redundant after execution, reduce or delete it instead of keeping parallel shapes alive.

## Public Contract To Establish

- `molt verify-service validate-config --config <path>` accepts a dedicated YAML file for the verify service only.
- The config contains:
  - one listener section
  - one source database section
  - one destination database section
- Each database section requires:
  - `url`
  - `tls.mode`
  - `tls.ca_cert_path`
  - optional `tls.client_cert_path`
  - optional `tls.client_key_path`
- `tls.mode` is explicit and restricted to:
  - `verify-full`
  - `verify-ca`
- Passwordless certificate-based auth is supported.
  - A database URL without a password is valid when the config includes a client certificate and client key pair.
- Certificate material pairing is strict.
  - client cert without client key is invalid
  - client key without client cert is invalid
- Listener protection is explicit.
  - `listener.transport.mode` is `http` or `https`
  - `listener.client_auth.mode` is `none` or `mtls`
  - `mtls` requires HTTPS plus `listener.tls.client_ca_path`
- When direct service authentication is disabled, the product says so clearly.
  - `validate-config` output should state that no extra built-in protection is being provided when `listener.client_auth.mode` is `none`
  - a copyable example config or fixture should make that same tradeoff explicit
- The verify-service command surface is config-only.
  - no `--source`
  - no `--target`
  - no `--source-url`
  - no `--target-url`
  - no listener TLS override flags
  - no verify-mode override flags

## Target YAML Shape

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  transport:
    mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_auth:
      mode: mtls
      client_ca_path: /config/certs/client-ca.crt
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb
    tls:
      mode: verify-full
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    url: postgresql://verify_target@crdb.internal:26257/appdb
    tls:
      mode: verify-ca
      ca_cert_path: /config/certs/destination-ca.crt
      client_cert_path: /config/certs/destination-client.crt
      client_key_path: /config/certs/destination-client.key
```

## Files And Structure To Add Or Change

- [x] `cockroachdb_molt/molt/cmd/root.go`
  - register the dedicated `verify-service` command namespace
- [x] `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - add `validate-config --config <path>` and keep the command config-only
- [x] `cockroachdb_molt/molt/verifyservice/config.go`
  - validated config types, enums, accessors, and one connection-render path
- [x] `cockroachdb_molt/molt/verifyservice/config_test.go`
  - public-behavior tests for valid and invalid config files plus warning output
- [x] `cockroachdb_molt/molt/verifyservice/testdata/*.yml`
  - copyable valid and invalid config fixtures covering the required modes
- [x] `cockroachdb_molt/molt/dbconn/config.go`
  - reduce, replace, or delete if it becomes redundant with the new canonical service connection shape

## TDD Execution Order

### Slice 1: Tracer Bullet For The Dedicated Service Config Surface

- [x] RED: add one failing Go command test that `molt verify-service validate-config --config <fixture>` succeeds for a minimal valid verify-service config and prints a stable summary
- [x] GREEN: add the `verify-service` command namespace plus the thinnest config loader needed to make that test pass
- [x] REFACTOR: keep command wiring thin and move all config behavior behind the verify-service config package

### Slice 2: Explicit Per-Database Verify Mode Contract

- [x] RED: add one failing config test proving that both source and destination TLS modes are required and restricted to `verify-full` or `verify-ca`
- [x] GREEN: add a small explicit enum and config validation for those two values only
- [x] REFACTOR: keep the enum and its parse/render behavior in one place rather than comparing raw strings throughout the package

### Slice 3: Certificate Material And Passwordless Client-Auth Support

- [x] RED: add one failing config test showing that passwordless URLs with client cert and key are accepted, while a lone cert or lone key is rejected
- [x] GREEN: validate client certificate/key pairing and preserve URL-based connectivity without requiring passwords in config
- [x] REFACTOR: expose one shared DB connection type for source and destination rather than duplicating certificate rules on both sides

### Slice 4: Listener HTTPS And Direct mTLS Boundary

- [x] RED: add one failing config test for listener protection rules:
  - `https` requires server cert and key
  - `mtls` requires HTTPS plus client CA
  - `http` plus `mtls` is invalid
- [x] GREEN: add the smallest listener transport and direct-auth enums that satisfy those rules
- [x] REFACTOR: keep listener TLS/client-auth ownership in the verify-service package instead of leaking it into `cmdutil` or a fake shared TLS helper

### Slice 5: Make Disabled Direct Service Auth Explicit

- [x] RED: add one failing test that `validate-config` output for `client_auth.mode: none` contains a clear "no extra built-in protection" statement
- [x] GREEN: surface that warning in the command summary and in a copyable example fixture or config comment
- [x] REFACTOR: keep the warning text derived from validated config state instead of hard-coding separate strings in tests and command wiring

### Slice 6: No Second Config Plane

- [x] RED: add one failing command-surface test that `molt verify-service validate-config --help` does not expose `--source`, `--target`, URL override flags, or verify-mode override flags
- [x] GREEN: keep the service command surface config-only and do not reuse `cmdutil.RegisterDBConnFlags`
- [x] REFACTOR: reduce or remove any old helper/type that exists only to carry source/target strings into the new service boundary

### Slice 7: Local Go Validation

- [x] RED: run the smallest focused Go test/package command that fails first for the new config boundary
- [x] GREEN: continue until the verify slice passes local Go validation, ending with `go test ./...` from `cockroachdb_molt/molt`
- [x] REFACTOR: if execution discovers the config package still depends on the wrong module boundary, flatten it before moving on

### Slice 8: Repository Validation Lanes

- [x] RED: run `make check`, `make lint`, and `make test`; fix only the first failing lane at a time
- [x] GREEN: continue until every required lane passes cleanly
- [x] REFACTOR: do one final `improve-code-boundaries` pass to confirm the service config stayed isolated from runner config and shared CLI flag baggage

## TDD Guardrails For Execution

- Start with a failing public-behavior test before changing the corresponding config rule.
- Keep tests focused on user-visible behavior:
  - accepted config shape
  - rejected config shape
  - stable validation output
  - command surface restrictions
- Do not invent the HTTP API early.
  - no job DTOs
  - no handler set
  - no in-memory job store
- Do not reintroduce verify config into the Rust runner YAML.
- Do not accept "optional" TLS verification mode defaults.
  - explicit means explicit here
- Do not grow the service command around `cmdutil.DBConnConfig`.
  - that is the old CLI boundary, not the service boundary
- Do not swallow bad config with warnings.
  - invalid config must fail loudly
- Do not run `make test-long` unless execution changes ignored-test selection or the task turns out to require that lane explicitly.

## Boundary Review Checklist

- [x] The verify-service config lives in the Go verify slice, not the Rust runner config
- [x] The service command surface is config-only and has no DB override flags
- [x] One validated DB connection shape is reused for source and destination
- [x] TLS verify mode is explicit and limited to `verify-full` or `verify-ca`
- [x] Passwordless cert-based auth works through config without fallback validation elsewhere
- [x] Listener direct-auth mode is explicit and `none` is clearly labeled as no extra built-in protection
- [x] No validation of listener or DB TLS material happens outside the config package
- [x] No duplicate source/target string carrier survives if it no longer owns a real boundary

## Final Verification For The Execution Turn

- [x] local Go validation for the verify-service config package and command
- [x] `make check`
- [x] `make lint`
- [x] `make test`
- [x] if execution changes ignored-test selection or the task explicitly requires it: `make test-long` was not required for this task
- [x] one final `improve-code-boundaries` pass after all required lanes are green
- [x] update the task file acceptance checkboxes and set `<passes>true</passes>` only after the required lanes pass

Plan path: `.ralph/tasks/story-18-verify-http-image/04-task-add-verify-service-config-for-source-destination-tls-and-mode-selection_plans/2026-04-19-verify-service-config-plan.md`

NOW EXECUTE
