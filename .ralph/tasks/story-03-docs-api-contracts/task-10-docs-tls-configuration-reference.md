## Task: Create unified TLS configuration reference document <status>completed</status> <passes>true</passes>

<description>
Must use tdd skill to complete

**Goal:** Produce a single reference document that explains TLS configuration for all components (runner webhook, runner destination, verify listener, verify database connections) in one place. Currently TLS is documented piecemeal across three README sections with different terminology and examples. A new user must mentally map `mode: verify-ca` to `sslmode=verify-ca` to `listener.tls.client_ca_path` without guidance.

**Exact things to include:**
- A new file `docs/tls-configuration.md` (or a substantial section in the README).
- A table showing each component and its TLS-relevant config fields:
  - Runner webhook: `mode` (http/https), `tls.cert_path`, `tls.key_path`
  - Runner destination: `tls.mode` (require/verify-ca/verify-full), `tls.ca_cert_path`, `tls.client_cert_path`, `tls.client_key_path`
  - Verify listener: `tls.cert_path`, `tls.key_path`, `tls.client_ca_path` (optional for mTLS)
  - Verify source/destination: `url` (contains `sslmode`), `ca_cert_path`, `client_cert_path`, `client_key_path`
- Explanation of each TLS mode:
  - `http` / no TLS: plain text, for local dev only
  - `https` / server TLS: server presents certificate, client verifies
  - `mTLS` / mutual TLS: both sides present and verify certificates
  - `require`: TLS enabled, no server cert verification
  - `verify-ca`: TLS enabled, verify server cert against CA
  - `verify-full`: TLS enabled, verify server cert and hostname
- Example config snippets for each common scenario:
  1. Runner webhook HTTP (local dev)
  2. Runner webhook HTTPS (production)
  3. Runner destination with verify-ca
  4. Runner destination with verify-full + client certs
  5. Verify listener HTTPS
  6. Verify listener mTLS
  7. Verify DB connection with sslmode=verify-full
- File path conventions: recommend `/config/certs/...` for containerized deployments.
- Cross-reference: point users to the OpenAPI spec for verify API endpoints and to the runner webhook payload docs.

**Exact things NOT to include:**
- OpenSSL command-line tutorials for generating certificates.
- Vendor-specific CA instructions (e.g., "how to get a cert from Let's Encrypt").
- Kubernetes Secret or Ingress configuration.
- Network architecture diagrams or firewall rules.
- Certificate format details (PEM vs DER) beyond "use PEM-encoded files".
- Internal implementation details about rustls or Go TLS libraries.
- Advice about cipher suites or TLS version negotiation.

**End result:**
A user who needs to configure TLS for any component can open one document, find their scenario in the examples, and copy-paste a working config without cross-referencing three different README sections.
</description>

<acceptance_criteria>
- [x] TLS reference document exists and covers all four TLS surfaces
- [x] Component-to-field mapping table is present and accurate
- [x] Each TLS mode is explained in plain language
- [x] At least 7 example config snippets cover common scenarios
- [x] File path convention `/config/certs/...` is recommended
- [x] No OpenSSL tutorials, vendor CA guides, or K8s configs are included
- [x] README links to the TLS reference document
- [x] `make check` — passes cleanly
- [x] `make test` — passes cleanly (default suite)
- [x] `make lint` — passes cleanly
</acceptance_criteria>

<plan>.ralph/tasks/story-03-docs-api-contracts/task-10-docs-tls-configuration-reference_plans/2026-04-25-tls-configuration-reference-plan.md</plan>
