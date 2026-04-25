## Task: Add optional HTTP mode to runner webhook listener <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete

**Goal:** Allow the runner webhook listener to operate over plain HTTP for local development and testing, while keeping HTTPS as the production default. Currently the runner in `crates/runner/src/webhook_runtime/mod.rs` unconditionally creates a TLS acceptor and serves HTTPS. A new user must generate certificates before they can test the webhook endpoint locally.

**In scope:**
- Add a `mode` field to `webhook` config in `crates/runner/src/config/mod.rs` and `crates/runner/src/config/parser.rs` with enum values `http` and `https` (default `https`).
- When `mode` is `http`, skip TLS setup and serve plain HTTP using `axum::serve` or `hyper_util` without TLS.
- When `mode` is `https` (default), keep existing behavior exactly.
- Make `webhook.tls` optional when `mode` is `http`, required when `mode` is `https`.
- Update README example to show HTTP mode for local dev.
- Update tests in `crates/runner/tests/config_contract.rs` to cover both modes.
- Update `ValidatedConfig` output to indicate the mode.

**Out of scope:**
- Changing the default from HTTPS to HTTP.
- Removing HTTPS support.
- Changing the verify service listener behavior.
- Changing destination TLS behavior.

**End result:**
A user can write:
```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
```
and start the runner without certificates for local testing. Production configs continue to work unchanged.
</description>

<acceptance_criteria>
- [ ] Config parser accepts `webhook.mode: http` and `webhook.mode: https`
- [ ] Config parser rejects `webhook.mode: https` without `webhook.tls` section
- [ ] Config parser accepts `webhook.mode: http` without `webhook.tls` section
- [ ] `validate-config` output includes the mode (e.g., `mode=https` or `mode=http`)
- [ ] Runner serves plain HTTP when mode is `http`
- [ ] Runner serves HTTPS when mode is `https` (existing behavior)
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
