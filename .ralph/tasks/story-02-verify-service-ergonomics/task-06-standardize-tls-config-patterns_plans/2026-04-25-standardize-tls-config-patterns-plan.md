# Plan: Standardize TLS Config Patterns Across Runner And Verify

## References

- Task:
  - `.ralph/tasks/story-02-verify-service-ergonomics/task-06-standardize-tls-config-patterns.md`
- Runner config and runtime boundaries:
  - `crates/runner/src/config/parser.rs`
  - `crates/runner/src/config/mod.rs`
  - `crates/runner/src/runtime_plan.rs`
  - `crates/runner/src/webhook_runtime/mod.rs`
- Runner public contracts:
  - `crates/runner/tests/config_contract.rs`
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/bootstrap_contract.rs`
  - `crates/runner/tests/reconcile_contract.rs`
- Verify config and CLI boundaries:
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/config_test.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Verify runtime/operator fixtures:
  - `cockroachdb_molt/molt/verifyservice/testdata/*`
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/runner/tests/support/verify_image_artifact_harness.rs`
- Operator docs:
  - `README.md`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient product approval for this planning turn.
- This turn is planning-only because task 06 had no existing plan artifact.
- This repo is explicitly greenfield with no backwards-compatibility requirement.
  - The task text mentions aliases if needed, but repo instructions say no backwards compatibility.
  - Execution should standardize the first-party config contract directly and update all owned fixtures/docs/tests in one pass.
  - If execution discovers a real blocker that makes a no-alias cut impossible, switch this plan back to `TO BE VERIFIED` and stop immediately.
- The verify service must keep URL-owned `sslmode`.
  - This task is about TLS path naming and YAML structure alignment, not about moving `sslmode` out of the URL.
- The runner webhook surface should match the verify listener surface closely enough that operators can transfer the same mental model.
  - `cert_path`
  - `key_path`
  - optional `client_ca_path`
- The runner destination and verify database surfaces should align around one visible YAML nesting pattern.
  - `url` remains at the database root for verify
  - certificate material moves under `tls`
  - runner destination keeps its decomposed `tls` block
- If the first RED slice shows that nested verify DB TLS materially worsens the public contract or conflicts with an already-proven image/runtime contract, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - verify accepts `verify.source.tls.*` and `verify.destination.tls.*` while preserving URL-owned `sslmode`
  - runner webhook accepts optional `webhook.tls.client_ca_path`
  - runner webhook runtime enforces client-cert verification when `client_ca_path` is configured
  - runner destination and verify database docs/tests show the same TLS field names and nesting pattern
  - README includes a side-by-side mapping table that makes listener and database TLS correspondence obvious
- Lower-priority concerns:
  - preserving the exact current verify flat YAML shape
  - preserving runner's current HTTPS-only listener vocabulary when `https+mtls` is the more honest operator summary

## Current State Summary

- Runner listener TLS is a two-field shape today:
  - `webhook.tls.cert_path`
  - `webhook.tls.key_path`
- Verify listener TLS is already the richer three-field shape:
  - `listener.tls.cert_path`
  - `listener.tls.key_path`
  - optional `listener.tls.client_ca_path`
- Runner destination decomposed config already uses a nested `tls` block with the target field names we want:
  - `ca_cert_path`
  - `client_cert_path`
  - `client_key_path`
- Verify database config still flattens TLS path material beside `url`.
  - the connection string builder then injects those paths into URL query params
  - that mixes two concerns in one DTO: operator YAML shape and connection-string mutation
- README currently documents runner and verify TLS as two related but visibly different contracts.
  - runner destination shows both URL-query TLS params and a decomposed `tls` alternative
  - verify database config shows flat TLS path fields at the database root
- Runner runtime currently always uses `with_no_client_auth()` for HTTPS, so adding `client_ca_path` is not just a parser change.

## Boundary Decision

- Standardize on one honest TLS sub-structure per connection boundary.
- Listener shape:
  - runner: `webhook.tls.{cert_path,key_path,client_ca_path?}`
  - verify: `listener.tls.{cert_path,key_path,client_ca_path?}`
- Database shape:
  - runner decomposed destination keeps:
    - `destination.tls.mode`
    - `destination.tls.ca_cert_path`
    - `destination.tls.client_cert_path`
    - `destination.tls.client_key_path`
  - verify source/destination becomes:
    - `verify.source.url`
    - `verify.source.tls.ca_cert_path`
    - `verify.source.tls.client_cert_path`
    - `verify.source.tls.client_key_path`
    - same for `verify.destination`
- Keep URL-owned policy separate from path-owned material.
  - verify `sslmode` stays in `url`
  - YAML `tls` carries only file-path material
- Prefer one focused TLS-material type on the verify side rather than keeping flat path fields on `DatabaseConfig`.
  - This is the boundary cleanup for the task: stop mixing connection policy in the URL with separate operator path fields at the same level.

## Improve-Code-Boundaries Focus

- Primary boundary smell to flatten:
  - verify database config currently mixes URL ownership and TLS-material ownership in one flat DTO
- Required cleanup during execution:
  - introduce one nested verify-side TLS material boundary so path fields live together under `tls`
  - keep `DatabaseConfig.ConnectionString()` responsible for enriching the URL with TLS file paths only
  - remove the old flat verify path field handling once all tests/fixtures/docs move to the new nested shape
  - extend the runner webhook TLS boundary once, then carry that richer shape through parser, runtime plan, and runtime instead of inventing a parallel mTLS config type
- Bold refactor allowance:
  - if a flat verify config field family becomes dead after the nested `tls` block lands, delete it instead of aliasing it
  - if `WebhookListenerTransport::Https { cert_path, key_path }` becomes an awkward partial duplicate, replace it with a single richer TLS struct instead of growing more parallel fields

## Intended Public Contract

- Runner listener:
  - HTTP remains `webhook.mode: http` with no `webhook.tls`
  - HTTPS remains `webhook.mode: https` with `webhook.tls.cert_path` and `webhook.tls.key_path`
  - mTLS becomes `webhook.mode: https` plus `webhook.tls.client_ca_path`
- Verify listener:
  - keep the current listener shape and terminology
- Runner destination decomposed config:
  - keep `tls.mode`
  - keep `tls.ca_cert_path`
  - keep `tls.client_cert_path`
  - keep `tls.client_key_path`
- Verify source/destination config:
  - move path fields under `tls`
  - keep `url` at the database root
  - reject the old flat path shape as obsolete once first-party fixtures are updated
- Docs:
  - show side-by-side listener TLS mapping and side-by-side database TLS mapping
  - make explicit that verify URL owns `sslmode` while the nested `tls` block owns file paths

## Files And Structure To Add Or Change

- `cockroachdb_molt/molt/verifyservice/config.go`
  - add a nested verify DB TLS material shape
  - update validation and `ConnectionString()` to read from the nested shape
- `cockroachdb_molt/molt/verifyservice/config_test.go`
  - replace flat verify DB fixtures/assertions with nested `tls` coverage
  - add explicit obsolete-shape rejection coverage if the first RED slice proves that is the expected contract
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - update CLI validation expectations and inline config fixtures
- `cockroachdb_molt/molt/verifyservice/testdata/*`
  - rewrite fixtures to the standardized nested shape
- `crates/runner/src/config/mod.rs`
  - extend listener TLS config with optional `client_ca_path`
- `crates/runner/src/config/parser.rs`
  - validate `webhook.tls.client_ca_path` when present
- `crates/runner/src/runtime_plan.rs`
  - carry the richer listener TLS shape through the runtime plan
- `crates/runner/src/webhook_runtime/mod.rs`
  - build rustls client verification when `client_ca_path` exists
- `crates/runner/src/error.rs`
  - add loud runtime errors for reading/parsing the client CA if needed
- `crates/runner/tests/config_contract.rs`
  - add/update config validation tests for runner webhook `client_ca_path`
  - keep runner destination TLS contract tests aligned with verify naming
- `crates/runner/tests/readme_operator_surface_contract.rs`
  - update README operator surface expectations and add side-by-side TLS table assertions
- `crates/runner/tests/verify_image_contract.rs`
  - keep verify image/config surface aligned with the new verify nested DB TLS shape
- `crates/runner/tests/support/verify_image_harness.rs`
  - materialize the new verify config structure
- `README.md`
  - add the side-by-side TLS comparison table
  - update inline runner and verify config examples to the new correspondence story

## Contract Decisions To Validate During Execution

- Runner webhook should expose an honest effective mode summary:
  - `http`
  - `https`
  - `https+mtls`
- Verify listener should keep the same vocabulary for effective mode summaries.
- Verify database validation messages should point at the nested fields:
  - `verify.source.tls.ca_cert_path`
  - `verify.source.tls.client_cert_path`
  - `verify.source.tls.client_key_path`
  - same for destination
- Runner destination naming should not regress to `sslrootcert` / `sslcert` / `sslkey` terminology in YAML.
- Old verify flat DB TLS fields should not remain as a hidden compatibility path unless execution proves a hard blocker.

## Vertical TDD Slices

### Slice 1: Tracer Bullet For Verify Nested Database TLS

- RED:
  - tighten one verify config test so a valid config uses `verify.source.tls.*`
  - require `ConnectionString()` to preserve URL-owned `sslmode` and inject the nested TLS file paths
- GREEN:
  - implement the smallest nested verify DB TLS shape that makes the test pass
- REFACTOR:
  - keep TLS file-path handling local to one nested helper/type instead of leaving flat fields on `DatabaseConfig`

### Slice 2: Reject Obsolete Verify Flat TLS Shape

- RED:
  - add one failing verify config test that uses the old flat `verify.source.ca_cert_path` style
  - require a loud invalid-config failure through the public loader
- GREEN:
  - rely on the new YAML shape and known-fields validation to reject obsolete flat fields
- REFACTOR:
  - delete or rewrite stale testdata fixtures rather than carrying both shapes

### Slice 3: Runner Webhook Accepts Optional Client CA

- RED:
  - add one failing runner config contract test proving `webhook.tls.client_ca_path` is accepted and surfaced honestly by validate-config output/logging
- GREEN:
  - extend the runner TLS config/parser/runtime-plan shape with optional `client_ca_path`
- REFACTOR:
  - replace partial duplicated HTTPS field plumbing with one richer webhook TLS struct if that removes duplication

### Slice 4: Runner Runtime Enforces mTLS When Configured

- RED:
  - add one failing runner public/runtime contract test that exercises HTTPS with `client_ca_path` and proves client certificates are required
- GREEN:
  - wire rustls client verification from the configured CA bundle
- REFACTOR:
  - keep HTTP and plain HTTPS paths untouched except for the richer shared listener TLS boundary

### Slice 5: Align Docs, Image Harnesses, And Operator Contracts

- RED:
  - tighten README contract tests and verify image contract tests to require:
    - side-by-side TLS mapping guidance
    - nested verify DB `tls` blocks
    - runner webhook `client_ca_path` support in the documented listener contract
- GREEN:
  - update README, inline examples, and harness-generated configs
- REFACTOR:
  - remove stale wording that teaches two competing mental models

### Slice 6: Final Lanes And Boundary Pass

- RED:
  - after the behavior slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long`
- GREEN:
  - continue until every required default lane passes
- REFACTOR:
  - do one final `improve-code-boundaries` pass and confirm the TLS contract became more uniform rather than more split

## TDD Guardrails For Execution

- One failing behavior slice at a time.
- Do not write all tests first.
- Test through public surfaces:
  - verify `LoadConfig()` and CLI validation
  - runner `validate-config` and runtime-facing contracts
- Do not add hidden alias parsing for obsolete verify field names.
- Do not move verify `sslmode` out of the URL.
- Do not reintroduce `sslrootcert`/`sslcert`/`sslkey` as YAML field names anywhere.
- If execution discovers the chosen nested verify shape or runner mTLS symmetry is wrong, switch this plan back to `TO BE VERIFIED` and stop immediately.

## Final Verification For The Execution Turn

- [ ] Verify source/destination config uses nested `tls` file-path fields
- [ ] Verify URL-owned `sslmode` still controls database verification mode
- [ ] Old verify flat DB TLS field names are removed from first-party configs/tests/docs
- [ ] Runner webhook accepts optional `client_ca_path`
- [ ] Runner runtime enforces mTLS when `client_ca_path` is configured
- [ ] Runner and verify listener docs use the same TLS field names
- [ ] Runner and verify database docs use the same TLS field names and visibly corresponding nesting
- [ ] README includes a side-by-side comparison table of TLS options
- [ ] All owned fixtures and contract tests are updated to the standardized contract
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] `make test-long` not run because this task is not a story-end lane
- [ ] Final `improve-code-boundaries` pass confirms the TLS boundary is cleaner than before
- [ ] Update the task file and set `<passes>true</passes>` only after all required lanes pass

Plan path: `.ralph/tasks/story-02-verify-service-ergonomics/task-06-standardize-tls-config-patterns_plans/2026-04-25-standardize-tls-config-patterns-plan.md`

NOW EXECUTE
