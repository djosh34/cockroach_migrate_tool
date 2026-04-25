# Plan: Create Unified TLS Configuration Reference Document

## References

- Task:
  - `.ralph/tasks/story-03-docs-api-contracts/task-10-docs-tls-configuration-reference.md`
- Current operator-facing docs:
  - `README.md`
  - `openapi/verify-service.yaml`
- Runner TLS public contract:
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/src/config/mod.rs`
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/support/readme_operator_surface.rs`
- Verify TLS public contract:
  - `cockroachdb_molt/molt/verifyservice/config_test.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - `cockroachdb_molt/molt/verifyservice/testdata/valid-http-listener.yml`
  - `cockroachdb_molt/molt/verifyservice/testdata/valid-https-server-tls.yml`
  - `cockroachdb_molt/molt/verifyservice/testdata/valid-https-mtls.yml`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the public documentation direction in this planning turn.
- This turn is planning-first because task 10 had no existing plan artifact.
- The right public boundary is a dedicated reference doc, not a large new README section:
  - `README.md` is already `1249` words against a `1250` word contract.
  - The README is already acting as a quick start, not a complete configuration manual.
- The execution target should therefore be `docs/tls-configuration.md`, with the README linking to it from the runner and verify quick-start areas.
- The doc must describe only the currently supported public config surface and must not invent new TLS knobs or mode names.
- If the first RED slice shows that the public contract cannot be expressed cleanly without changing the actual runtime config shape, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - a dedicated TLS reference doc exists at `docs/tls-configuration.md`
  - it covers all four TLS surfaces:
    - runner webhook listener
    - runner destination database
    - verify listener
    - verify source and destination databases
  - it contains an accurate component-to-field mapping table
  - it explains each required mode in plain language:
    - `http`
    - `https`
    - `mTLS`
    - `require`
    - `verify-ca`
    - `verify-full`
  - it includes all seven required copy-pasteable example snippets
  - it recommends `/config/certs/...` file paths for containerized deployment
  - it cross-references:
    - `openapi/verify-service.yaml`
    - runner webhook payload docs in `README.md`
  - the README links to the TLS reference doc without bloating the quick start
- Lower-priority concerns:
  - preserving the existing README TLS mapping table if it becomes duplicate noise
  - keeping every scattered TLS sentence in the README once the dedicated reference exists

## Current State Summary

- TLS guidance is currently split across multiple places in `README.md`:
  - runner quick start has HTTP vs HTTPS listener guidance
  - runner quick start has destination URL TLS query params and one explicit-field alternative
  - runner quick start has a small runner-vs-verify TLS field mapping table
  - verify quick start has one mixed HTTPS and DB TLS example
- There is no dedicated `docs/` directory or dedicated TLS reference file yet.
- The current README already links operators to two neighboring public contracts that this new doc should reference instead of duplicating:
  - `openapi/verify-service.yaml`
  - `### Webhook Payload Format`
- Runner and verify tests already define the honest config surface we must document:
  - runner webhook uses `mode: http` or `mode: https`, with `webhook.tls.cert_path`, `webhook.tls.key_path`, and optional `webhook.tls.client_ca_path`
  - runner destinations support either URL query params such as `sslmode`, `sslrootcert`, `sslcert`, and `sslkey`, or explicit `destination.tls.*` fields including `mode`
  - verify listener uses `listener.tls.cert_path`, `listener.tls.key_path`, and optional `listener.tls.client_ca_path`
  - verify source and destination use `url` for `sslmode` and nested `tls` file-path fields for CA and client cert material
- There is no single test owner for the unified TLS reference doc yet.

## Boundary Decision

- Create one dedicated document, `docs/tls-configuration.md`, as the single owner of detailed TLS operator guidance.
- Keep `README.md` focused on quick-start execution:
  - short TLS mentions stay
  - detailed comparison tables and scenario walkthroughs move to the dedicated doc
  - the README should link operators to the doc instead of carrying a second full reference
- Add one dedicated docs contract test instead of stuffing all TLS assertions into the existing README contract test.
- The docs contract test should own:
  - required sections and cross-references
  - required component mapping rows
  - required examples and mode explanations
  - required exclusions

## Improve-Code-Boundaries Focus

- Primary boundary smell to flatten:
  - TLS knowledge currently lives in the wrong place by being split between runner quick-start prose, verify quick-start prose, and one small duplicated mapping table.
- Required cleanup during execution:
  - move full TLS reference content into `docs/tls-configuration.md`
  - reduce the README to short entry-point links and minimal quick-start hints
  - remove or replace the duplicated README TLS field mapping table if it becomes redundant after the dedicated doc exists
  - keep the contract for the new reference in a dedicated helper/test pair instead of spreading raw markdown checks across unrelated tests
- If execution finds a tiny one-caller helper added only to hide trivial markdown parsing, inline it rather than growing fake boundaries.

## Intended Public Contract

- Create `docs/tls-configuration.md` with a stable operator-facing structure such as:
  - `# TLS Configuration Reference`
  - overview of the four TLS surfaces
  - component-to-field mapping table
  - mode explanations
  - common scenario examples
  - cross-references
- The mapping table must explicitly cover:
  - runner webhook: `mode`, `tls.cert_path`, `tls.key_path`
  - runner destination: `tls.mode`, `tls.ca_cert_path`, `tls.client_cert_path`, `tls.client_key_path`
  - verify listener: `tls.cert_path`, `tls.key_path`, `tls.client_ca_path`
  - verify source/destination: `url` with `sslmode`, plus `ca_cert_path`, `client_cert_path`, `client_key_path`
- The mode explanations must stay plain-language and operator-facing:
  - no rustls or Go TLS implementation details
  - no OpenSSL tutorials
  - no vendor CA instructions
  - no Kubernetes Secret or Ingress content
  - no cipher-suite or TLS-version advice
- Required scenario examples:
  - runner webhook HTTP for local development
  - runner webhook HTTPS for production
  - runner destination using `verify-ca`
  - runner destination using `verify-full` with client certificates
  - verify listener HTTPS
  - verify listener mTLS
  - verify DB connection using `sslmode=verify-full`
- Path convention guidance must consistently recommend PEM files mounted under `/config/certs/...`.
- The README should link to this document from the runner and verify quick-start areas so a new operator can find it from either side.

## Files And Structure To Add Or Change

- `docs/tls-configuration.md`
  - new canonical TLS reference doc
- `README.md`
  - add concise links to the TLS reference doc
  - remove or shrink duplicated TLS comparison prose if needed
- `crates/runner/tests/tls_reference_contract.rs`
  - new public contract test for the dedicated TLS reference doc
- `crates/runner/tests/support/tls_reference_surface.rs`
  - helper for loading the doc and extracting its sections in one place
- `crates/runner/tests/readme_operator_surface_contract.rs`
  - minimal assertion that the README links to the TLS reference doc, if that is not better owned entirely by the new contract test

## Vertical TDD Slices

### Slice 1: Tracer Bullet For The Dedicated TLS Reference

- RED:
  - add a failing docs contract that requires:
    - `docs/tls-configuration.md` exists
    - it starts with `# TLS Configuration Reference`
    - it recommends `/config/certs/...`
    - the README links to it
- GREEN:
  - create the docs directory and the new file with the title, a short intro, the path convention note, and minimal README links
- REFACTOR:
  - add the dedicated doc-surface helper so the contract stops doing raw ad hoc file reads

### Slice 2: Component Mapping Table And Mode Explanations

- RED:
  - tighten the docs contract to require:
    - one accurate component-to-field mapping table covering all four TLS surfaces
    - plain-language explanations for `http`, `https`, `mTLS`, `require`, `verify-ca`, and `verify-full`
  - require the doc to avoid the forbidden implementation-detail content
- GREEN:
  - write the mapping table and mode explanation section in the dedicated doc
- REFACTOR:
  - remove the duplicated README TLS field mapping table if the new doc now owns that comparison better

### Slice 3: Runner-Side Examples And Cross-Reference Links

- RED:
  - tighten the docs contract to require examples for:
    - runner webhook HTTP
    - runner webhook HTTPS
    - runner destination with `verify-ca`
    - runner destination with `verify-full` plus client certs
  - require a cross-reference to the runner webhook payload docs
- GREEN:
  - add the runner-side examples and the webhook payload cross-reference
- REFACTOR:
  - compress nearby README TLS prose so the quick start stays clean and the dedicated doc is the obvious detail owner

### Slice 4: Verify-Side Examples And Verify API Cross-Reference

- RED:
  - tighten the docs contract to require examples for:
    - verify listener HTTPS
    - verify listener mTLS
    - verify DB connection with `sslmode=verify-full`
  - require a cross-reference to `openapi/verify-service.yaml`
  - require explicit absence of OpenSSL tutorials, vendor CA guidance, Kubernetes config, and internal library details
- GREEN:
  - add the verify-side examples and OpenAPI cross-reference
- REFACTOR:
  - trim any redundant verify TLS prose in the README if the new doc now covers it more honestly

### Slice 5: Final Lanes And Boundary Pass

- RED:
  - after all docs slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long`
- GREEN:
  - continue until every required default lane passes
- REFACTOR:
  - do one final `improve-code-boundaries` pass and confirm the TLS reference now has one clear documentation owner

## TDD Guardrails For Execution

- One failing slice at a time.
- Do not bulk-write all TLS assertions first.
- Test through public surfaces only:
  - `docs/tls-configuration.md`
  - `README.md`
  - existing public config contracts already encoded in runner and verify tests
- Do not document config fields or mode semantics that the code does not support today.
- Do not widen the README beyond its quick-start boundary just to satisfy this task.
- If execution discovers a real mismatch between the documented TLS surface and the implemented config contract, switch this plan back to `TO BE VERIFIED` and stop instead of papering over the mismatch.

## Final Verification For The Execution Turn

- [ ] `docs/tls-configuration.md` exists and is the canonical TLS reference
- [ ] The doc covers runner webhook, runner destination, verify listener, and verify source/destination
- [ ] The component-to-field mapping table is present and accurate
- [ ] Each mode explanation is present and plain-language
- [ ] All 7 required example snippets are present
- [ ] `/config/certs/...` is the recommended path convention
- [ ] The doc cross-references `openapi/verify-service.yaml`
- [ ] The doc cross-references the runner webhook payload docs
- [ ] The doc excludes OpenSSL tutorials, vendor CA guides, Kubernetes config, and implementation-detail TLS internals
- [ ] The README links to the TLS reference doc
- [ ] Duplicate TLS prose in the README is reduced or removed where the dedicated doc now owns it
- [ ] Dedicated TLS reference contract tests pass
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long` not run because this is not a story-end task
- [ ] Final `improve-code-boundaries` pass confirms the TLS documentation boundary is cleaner than before
- [ ] Update the task file and set `<passes>true</passes>` only after all required lanes pass

Plan path: `.ralph/tasks/story-03-docs-api-contracts/task-10-docs-tls-configuration-reference_plans/2026-04-25-tls-configuration-reference-plan.md`

NOW EXECUTE
