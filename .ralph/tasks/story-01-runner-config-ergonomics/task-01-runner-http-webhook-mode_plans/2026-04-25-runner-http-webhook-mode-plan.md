# Plan: Add Optional HTTP Mode To The Runner Webhook Listener

## References

- Task:
  - `.ralph/tasks/story-01-runner-config-ergonomics/task-01-runner-http-webhook-mode.md`
- Current runner webhook config boundary:
  - `crates/runner/src/config/mod.rs`
  - `crates/runner/src/config/parser.rs`
  - `crates/runner/src/lib.rs`
- Current runtime transport boundary:
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
  - `crates/runner/src/error.rs`
- Current contract coverage:
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/webhook_contract.rs`
  - `crates/runner/tests/readme_operator_surface_contract.rs`
- Current operator docs and fixtures:
  - `README.md`
  - `crates/runner/tests/fixtures/valid-runner-config.yml`
  - `crates/runner/tests/fixtures/container-runner-config.yml`
  - `crates/runner/tests/fixtures/readme-runner-config.yml`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient product approval for this planning turn.
- This turn is planning-only because the task had no linked plan artifact yet.
- This is greenfield work.
  - keep HTTPS as the default
  - add HTTP only as an explicit operator choice
  - remove awkward TLS-hardcoded seams instead of preserving them
- The runtime should have one webhook listener transport decision, not parallel booleans or optional-path conventions.
- `axum::serve` or a similarly direct non-TLS path is sufficient for the HTTP listener mode.
- If the first RED slice proves the listener contract needs more than two honest modes, or that HTTP and HTTPS need materially different runtime abstractions beyond one transport enum, switch this plan back to `TO BE VERIFIED` and stop immediately.
- If the first RED slice proves the current public contract cannot represent TLS-required validation without muddying the config model, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - `validate-config` accepts `webhook.mode: http` without `webhook.tls`
  - `validate-config` keeps defaulting to `mode=https`
  - `validate-config` rejects `mode=https` without TLS material
  - `validate-config` output includes the selected mode
  - the runner really serves plain HTTP in `mode=http`
  - the runner keeps serving HTTPS exactly as before in `mode=https`
- Lower-priority concerns:
  - preserving unconditional `tls=` text in validation output
  - keeping HTTPS-only README examples as the default local-dev path

## Current State Summary

- `RawWebhookConfig` currently requires:
  - `bind_addr`
  - `tls`
- `WebhookConfig` mirrors that hard requirement:
  - `bind_addr: SocketAddr`
  - `tls: TlsConfig`
- `ValidatedConfig` always renders:
  - `webhook=<addr>`
  - `tls=<cert+key>`
- `RunnerStartupPlan` and `RunnerRuntimePlan` always carry:
  - `tls_cert_path`
  - `tls_key_path`
- `webhook_runtime::serve` always:
  - loads rustls config
  - creates `TlsAcceptor`
  - performs TLS handshakes before serving requests
- README runner quick start currently instructs users to generate and mount server certificates before local use.
- The nearest existing contract tests are HTTPS-shaped:
  - config tests assert `tls=...` in validate-config output
  - bootstrap tests probe runner health via an HTTPS client
  - README operator-surface tests materialize an HTTPS runner example

## Boundary Decision

- Introduce an explicit webhook transport contract in the config/runtime model instead of smearing TLS paths through multiple structs.
- Preferred public config shape:

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
```

or:

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

- Preferred internal shape:
  - `WebhookMode` enum with `Http` and `Https`
  - one webhook listener config/module that owns:
    - bind address
    - mode
    - optional TLS material only when mode is HTTPS
- Avoid this muddier shape:
  - `https: bool`
  - `tls: Option<TlsConfig>`
  - scattered `if tls.is_some()` checks across parser, validation output, startup plan, and runtime serve loop
- Validation rule:
  - `mode` defaults to `https`
  - `tls` is required for `https`
  - `tls` must be absent or ignored loudly for `http`
- Keep the public contract explicit.
  - do not infer HTTP from a missing `tls` section alone
  - do not add legacy aliases like `transport`, `scheme`, or `allow_insecure`

## Improve-Code-Boundaries Focus

- Primary smell to flatten:
  - TLS file paths are currently treated as unconditional runtime state even though they only exist for one listener mode.
- Required cleanup during execution:
  - move webhook transport ownership behind one config/runtime boundary
  - stop carrying TLS cert/key paths as top-level runtime plan fields when HTTP mode has no TLS material
  - keep `webhook_runtime::serve` mode-driven instead of constructing one app and then scattering TLS conditionals around accept loops
  - make validation output reflect the listener mode directly instead of implying HTTPS by the presence of `tls=...`
- Bold refactor allowance:
  - if `RunnerStartupPlan` and `RunnerRuntimePlan` should each store a `WebhookListenerPlan` or similar instead of three parallel fields, do that
  - if `TlsConfig::material_label()` or other helpers become mode-specific baggage, move or remove them

## Error And Output Contract Decisions

- Keep config failures field-specific:
  - missing TLS for HTTPS should point at `webhook.tls`
  - invalid `mode` should point at `webhook.mode`
- `ValidatedConfig` should always expose `mode=<http|https>`.
- Text output expectations:
  - HTTP mode should not print a fake TLS label
  - HTTPS mode may continue to print TLS material detail, but only as a transport-specific field
- JSON log expectations:
  - the `config.validated` event should include `mode`
  - include `tls` only when the config actually has TLS material

## Intended Files And Structure To Add Or Change

- `crates/runner/src/config/mod.rs`
  - add `WebhookMode`
  - refactor `WebhookConfig` around explicit mode ownership
- `crates/runner/src/config/parser.rs`
  - parse `webhook.mode` with default `https`
  - make `webhook.tls` conditional on mode
  - reject invalid mode/TLS combinations loudly
- `crates/runner/src/lib.rs`
  - update `ValidatedConfig` to expose the mode and conditionally render TLS information
- `crates/runner/src/runtime_plan.rs`
  - replace unconditional webhook TLS path fields with one mode-owned listener plan
- `crates/runner/src/webhook_runtime/mod.rs`
  - serve plain HTTP when mode is `http`
  - keep the existing TLS path for `https`
- `crates/runner/src/error.rs`
  - only if needed for a clearer field-specific config error contract
- `crates/runner/tests/config_contract.rs`
  - cover both modes and validation output
- `crates/runner/tests/bootstrap_contract.rs`
  - add a real HTTP listener runtime slice
  - keep an HTTPS runtime slice proving existing behavior still works
- `README.md`
  - show HTTP mode for local development
  - keep HTTPS documented as the production-default path
- `crates/runner/tests/fixtures/readme-runner-config.yml`
  - align the README-owned runner example with the local-dev HTTP mode
- Optional only if execution proves necessary:
  - `crates/runner/tests/webhook_contract.rs`
    - only if a narrower end-to-end runtime assertion belongs there instead of `bootstrap_contract.rs`
  - `crates/runner/tests/fixtures/valid-runner-config.yml`
    - only if the canonical config fixture should assert the default `https` mode explicitly or via omission

## Public Contract Decisions

- Supported webhook YAML shapes:

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
```

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

- The third form above remains valid and means `mode=https` by default.
- Unsupported shapes:

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: https
```

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

- Unsupported shapes must fail loudly instead of guessing operator intent.

## Vertical TDD Slices

### Slice 1: Tracer Bullet For HTTP Config Acceptance

- RED:
  - add one failing `config_contract.rs` test that writes a minimal runner config with `webhook.mode: http` and no `webhook.tls`
  - require `validate-config` to succeed and print `mode=http`
- GREEN:
  - add the smallest parser/model/output change needed for honest HTTP acceptance
- REFACTOR:
  - move mode ownership into `WebhookConfig` instead of leaving parser-only branching

### Slice 2: Default HTTPS And TLS-Required Validation

- RED:
  - add one failing config contract test that omits `mode` but includes TLS and requires `mode=https` in output
  - add one failing config contract test that sets `mode: https` without `webhook.tls` and requires a clear validation failure
- GREEN:
  - implement defaulting plus TLS-required validation
- REFACTOR:
  - keep the conditional TLS rule in one place instead of duplicating it across raw structs and runtime planning

### Slice 3: Reject HTTP Plus TLS Material

- RED:
  - add one failing config contract test for `mode: http` plus a `webhook.tls` section
  - require a loud failure rather than silently ignoring the extra fields
- GREEN:
  - implement explicit rejection
- REFACTOR:
  - avoid a half-optional config model that keeps dead TLS state around for HTTP mode

### Slice 4: Validation Event And Text Output Surface

- RED:
  - add or update config contract coverage for plain-text and JSON `validate-config` output
  - require `mode` to appear in both surfaces
  - require TLS detail to remain transport-specific instead of unconditional
- GREEN:
  - update `ValidatedConfig`
- REFACTOR:
  - keep output formatting derived from the webhook transport contract rather than hand-built string conditionals

### Slice 5: Real HTTP Runtime Path

- RED:
  - add one failing `bootstrap_contract.rs` integration that writes a runner config with `mode: http`
  - probe `/healthz` over plain HTTP and require startup without certificates
- GREEN:
  - implement the non-TLS listener path in `webhook_runtime::serve`
- REFACTOR:
  - share the router/app setup while keeping transport-specific accept logic honest and shallow

### Slice 6: HTTPS Runtime Regression Guard

- RED:
  - add or tighten one HTTPS runtime assertion proving the existing TLS listener behavior still works with either defaulted or explicit `mode: https`
- GREEN:
  - fix any regressions introduced by the transport split
- REFACTOR:
  - ensure the HTTPS path still owns TLS setup cleanly rather than becoming a special case under an HTTP-first design

### Slice 7: README And README-Owned Fixture Contract

- RED:
  - add failing README/operator-surface assertions as needed so local runner docs show `mode: http`
  - keep production/default HTTPS guidance explicit in surrounding prose
- GREEN:
  - update `README.md` and `readme-runner-config.yml`
- REFACTOR:
  - remove stale “must generate certs before local testing” guidance from the default local-dev flow

### Slice 8: Final Lanes And Boundary Pass

- RED:
  - after the behavior slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long`
- GREEN:
  - continue until every required default lane passes
- REFACTOR:
  - do one final `improve-code-boundaries` pass and confirm the webhook listener boundary is smaller and more honest than before

## TDD Guardrails For Execution

- One failing behavior slice at a time.
- Do not add tests after implementation for the same behavior.
- Test through public runner surfaces first:
  - `validate-config`
  - real runner startup and `/healthz`
  - README-owned operator docs
- Do not silently treat missing TLS as HTTP.
- Do not silently ignore `webhook.tls` when `mode=http`.
- Do not keep unconditional TLS runtime fields if HTTP mode makes them optional.
- If execution discovers the transport enum is the wrong boundary, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Final Verification For The Execution Turn

- [ ] Red/green TDD covers HTTP acceptance, HTTPS defaulting, HTTPS-without-TLS rejection, and HTTP-with-TLS rejection
- [ ] `validate-config` output includes `mode=http|https`
- [ ] Runner serves plain HTTP without certificates when `mode=http`
- [ ] Runner still serves HTTPS with the existing TLS behavior when `mode=https`
- [ ] README documents HTTP mode for local development while keeping HTTPS as the default production posture
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long` not run because this task does not require the long lane
- [ ] Final `improve-code-boundaries` pass confirms the webhook transport boundary got simpler rather than muddier
- [ ] Update the task file and set `<passes>true</passes>` only after all required lanes pass

Plan path: `.ralph/tasks/story-01-runner-config-ergonomics/task-01-runner-http-webhook-mode_plans/2026-04-25-runner-http-webhook-mode-plan.md`

NOW EXECUTE
