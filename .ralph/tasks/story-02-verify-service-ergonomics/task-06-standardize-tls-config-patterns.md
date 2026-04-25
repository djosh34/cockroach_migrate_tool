## Task: Standardize TLS config field naming and structure across runner and verify <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete

**Goal:** Make TLS configuration look the same across runner and verify service so users do not need to learn two different mental models. Currently the runner uses `webhook.tls.cert_path` and `mappings[].destination.tls.mode` with a YAML enum, while verify uses `listener.tls.cert_path` and `verify.source.url` with `sslmode` in the URL query string. The field names and nesting differ.

**In scope:**
- Align field names: ensure both components use `cert_path`, `key_path`, `ca_cert_path`, `client_cert_path`, `client_key_path` in the same YAML nesting structure where applicable.
- For runner destination TLS: keep the decomposed YAML structure but ensure field names exactly match verify's convention (e.g., `ca_cert_path` instead of mixing `sslrootcert` concepts).
- For runner webhook TLS: keep `webhook.tls.cert_path` and `key_path` but consider if `client_ca_path` should be added for mTLS parity with verify.
- For verify service: keep existing structure but rename or document so the mapping to runner fields is obvious.
- Update README to show side-by-side examples of equivalent TLS configs.
- Update all test fixtures and config contract tests.
- Do NOT change the verify service URL-based `sslmode` (the user explicitly wants to keep URL-based sslmode for verify DB connections).

**Out of scope:**
- Unifying into a single config file.
- Changing verify's URL-based `sslmode` behavior.
- Removing any existing TLS modes.
- Changing certificate file formats or generation.

**End result:**
A user can look at runner TLS config and verify TLS config and immediately see which fields correspond, without guessing about naming differences.
</description>

<acceptance_criteria>
- [ ] Runner webhook TLS field names are consistent with verify listener TLS field names
- [ ] Runner destination TLS field names are consistent with verify database TLS field names where applicable
- [ ] README includes a side-by-side comparison table of TLS options
- [ ] All existing config fixtures and tests updated to use standardized names
- [ ] No breaking changes to working configs (backward-compatible aliases if needed)
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
