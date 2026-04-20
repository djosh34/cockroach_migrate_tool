# Plan: Reset Verify-Service Security Contract To Operator-Chosen Modes

## References

- Task:
  - `.ralph/tasks/story-27-verify-operator-ux-reset/01-task-reset-verify-service-config-to-operator-chosen-security-and-remove-redundant-tls-knobs.md`
- Current verify-service config/runtime boundary:
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/runtime.go`
  - `cockroachdb_molt/molt/verifyservice/service.go`
  - `cockroachdb_molt/molt/verifyservice/raw_table.go`
- Current Go contracts:
  - `cockroachdb_molt/molt/verifyservice/config_test.go`
  - `cockroachdb_molt/molt/verifyservice/runtime_test.go`
  - `cockroachdb_molt/molt/verifyservice/verify_runner_test.go`
  - `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- Current operator-facing docs and image contracts:
  - `README.md`
  - `crates/runner/tests/verify_image_contract.rs`
  - `crates/runner/tests/readme_operator_surface_contract.rs`
  - `crates/runner/tests/support/verify_image_harness.rs`
  - `crates/runner/tests/support/operator_cli_surface.rs`
- Skills:
  - `tdd`
  - `improve-code-boundaries`

## Planning Assumptions

- The task markdown is sufficient approval for the product direction in this planning turn.
- This turn is planning-only because the task has no existing plan artifact yet.
- The public contract has already drifted into policy-owned nesting:
  - listener security is split across `transport.mode`, `tls`, and `client_auth.mode`
  - database verification mode is duplicated in both the URL query and YAML `tls.mode`
  - `raw_table_output.enabled` keeps a nested-toggle shape even though the service only consumes one boolean
- This is greenfield work with no backwards-compatibility requirement.
  - remove awkward config shapes rather than translating them forward
  - delete obsolete fixture/config variants once replacement coverage exists
- If the first RED slice shows the planned YAML contract cannot express the supported operator modes without hidden ambiguity, this plan must be switched back to `TO BE VERIFIED` and execution must stop immediately.

## Approval And Test Priorities

- Highest-priority behaviors to prove:
  - config parsing accepts plain HTTP, HTTPS without mTLS, and HTTPS with mTLS
  - runtime startup does not reject the service solely because the operator chose HTTP or no client authentication
  - source and destination database verification behavior comes from exactly one source of truth each
  - CLI validation output and README guidance make the supported listener modes obvious without security-shaming or fake requirements
  - the verify image and README fixtures stop encoding the old “HTTPS + mTLS only” contract
- Lower-priority concerns:
  - exact wording of explanatory log/help text once the supported modes are honest
  - preserving any old nested config shape for migration purposes

## Current State Summary

- The current listener contract is policy-heavy and rejects operator choice:
  - `listener.transport.mode` only accepts `https`
  - `listener.tls.client_auth.mode` only accepts `mtls`
  - validation requires a client CA whenever HTTPS is selected
  - runtime always loads a TLS config and always calls `ListenAndServeTLS`
- The current database TLS contract is duplicated:
  - operators must embed `sslmode` semantics in the URL anyway
  - `DatabaseConfig.ConnectionString()` then overrides `sslmode` again from YAML `tls.mode`
  - tests and README examples currently encode both copies of the same decision
- The current config shape is deeper than the runtime needs:
  - listener mode is expressed through two enums and two nested structs when the runtime only needs to know whether TLS exists and whether client cert verification is configured
  - database cert material is hidden under `verify.{source,destination}.tls.*` even though the runner/raw-table code only needs one connection boundary per database
  - `raw_table_output.enabled` is the only part of that nested shape actually consumed by `Service`
- The current operator-facing contracts reinforce the wrong policy:
  - README examples imply HTTPS+mTLS is required
  - verify image harness writes only the HTTPS+mTLS config shape
  - validate-config output reports listener transport and client-auth enums instead of the operator-visible effective mode

## Boundary Decision

- Flatten the config around real runtime decisions instead of policy enums.
- Preferred listener contract:
  - `listener.bind_addr`
  - optional `listener.tls` block
  - `listener.tls.cert_path`
  - `listener.tls.key_path`
  - optional `listener.tls.client_ca_path`
- Meaning of that contract:
  - no `listener.tls` block means plain HTTP
  - `listener.tls` with cert/key only means HTTPS without mTLS
  - `listener.tls` with `client_ca_path` means HTTPS with mTLS
- Preferred database contract:
  - `verify.source.url`
  - `verify.source.ca_cert_path`
  - `verify.source.client_cert_path`
  - `verify.source.client_key_path`
  - same flattened shape for `verify.destination`
  - no `verify.*.tls.mode`
  - the URL query owns `sslmode` and therefore owns the verification policy
- Preferred raw-table toggle cleanup:
  - replace `verify.raw_table_output.enabled` with one direct config boolean if the feature remains configurable
  - if execution shows the feature should simply always be on for this product surface, delete the toggle completely instead of preserving a dead config branch

## Improve-Code-Boundaries Focus

- Primary boundary smell to flatten:
  - security posture is currently represented as multiple nested DTOs and enums instead of one honest runtime contract
- Required cleanup during execution:
  - delete listener transport/client-auth enums if the capability-based listener contract replaces them
  - collapse `DatabaseTLSConfig` into the database boundary or remove it entirely if only cert-material fields remain
  - remove duplicate config-to-connection-string policy decisions so `ConnectionString()` only enriches the URL with cert material, not a second verification mode
  - flatten or remove `RawTableOutputConfig` so the service owns one direct boolean instead of a nested `enabled` wrapper
- Bold refactor allowance:
  - if `ListenerTransportConfig`, `ListenerClientAuthConfig`, `DatabaseTLSConfig`, or `RawTableOutputConfig` become pass-through shells after the new contract lands, delete the types and update tests/fixtures to the smaller shape

## Intended Public Contract

- Listener modes:
  - plain HTTP is a supported operator choice
  - HTTPS without client-auth is a supported operator choice
  - HTTPS with client certificate verification is a supported operator choice
  - startup may explain the chosen mode, but must not reject HTTP or no-mTLS solely because they are less secure
- Listener validation:
  - HTTP requires only `bind_addr`
  - HTTPS requires `cert_path` and `key_path`
  - mTLS requires `client_ca_path`
  - invalid partial TLS configuration must fail loudly with direct field-level errors
- Database verification:
  - `sslmode` comes from the database URL and is not repeated anywhere else
  - YAML only supplies cert material that the runtime actually injects into the connection string
  - client cert/key pairing rules remain enforced
  - invalid URL schemes still fail loudly
- Operator surface:
  - `validate-config` text and JSON output should expose the effective listener mode and the source/destination DB connection policy without pointing at deleted nested knobs
  - README and fixture configs must show the smaller YAML shape and clearly call out the three supported listener modes
  - verify image harness/config contracts must stop requiring mTLS as the only valid startup shape

## Files And Structure To Add Or Change

- `cockroachdb_molt/molt/verifyservice/config.go`
  - flatten listener and database config structures
  - move validation to the new direct contract
- `cockroachdb_molt/molt/verifyservice/runtime.go`
  - start HTTP or HTTPS based on the presence of listener TLS config
  - keep client-cert enforcement optional inside the TLS path
- `cockroachdb_molt/molt/verifyservice/service.go`
  - consume a flattened raw-table toggle or remove it if the feature becomes unconditional
- `cockroachdb_molt/molt/verifyservice/raw_table.go`
  - keep consuming database connection strings through the single-source-of-truth DB boundary
- `cockroachdb_molt/molt/verifyservice/config_test.go`
  - replace old rejection tests with mode-acceptance and flattening tests
- `cockroachdb_molt/molt/verifyservice/runtime_test.go`
  - cover HTTP, HTTPS, and HTTPS+mTLS startup paths
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
  - update validation summaries/log fields to the new contract
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
  - update help and validation expectations
- `README.md`
  - replace the old nested verify-service example and describe the supported modes honestly
- `crates/runner/tests/verify_image_contract.rs`
  - keep the image/operator contract aligned with the new supported surface
- `crates/runner/tests/readme_operator_surface_contract.rs`
  - update README expectations once the verify quick-start changes
- `crates/runner/tests/support/verify_image_harness.rs`
  - materialize the smaller config shape and stop encoding duplicated TLS mode knobs
- `crates/runner/tests/support/operator_cli_surface.rs`
  - update help-contract markers if the visible operator surface changes
- `cockroachdb_molt/molt/verifyservice/testdata/*`
  - replace old nested fixtures with the new flattened contract and add explicit HTTP / HTTPS / HTTPS+mTLS coverage

## Vertical TDD Slices

### Slice 1: Tracer Bullet For Plain HTTP Acceptance

- RED:
  - add one failing config/runtime test pair that proves a minimal HTTP listener config is accepted and can start the service without TLS
  - make the test exercise the public config loader and runtime entrypoint, not private helper internals
- GREEN:
  - implement the smallest config/runtime change needed to accept HTTP and call the correct server start path
- REFACTOR:
  - remove policy-only listener enums if they are now dead weight

### Slice 2: Prove HTTPS Without mTLS Is A First-Class Mode

- RED:
  - add one failing test that accepts server cert/key without `client_ca_path`
  - prove the runtime serves HTTPS successfully without requiring a client certificate
- GREEN:
  - make listener TLS validation require only cert/key for HTTPS
- REFACTOR:
  - keep TLS config loading isolated to the TLS branch; do not force HTTP through TLS setup

### Slice 3: Preserve HTTPS With mTLS As An Optional Stronger Mode

- RED:
  - add the next failing test that accepts `client_ca_path` and requires client cert verification at runtime
- GREEN:
  - keep or adapt the existing mTLS runtime path under the flattened listener contract
- REFACTOR:
  - centralize listener mode reporting so validation output, logs, and tests agree on one vocabulary

### Slice 4: Remove Duplicate Database Verification Knobs

- RED:
  - add a failing config/connection-string contract test that uses URL-owned `sslmode` and asserts YAML no longer has a second mode field
  - include one URL using `verify-full` and one using `verify-ca`
- GREEN:
  - flatten the DB config shape, remove `tls.mode`, and make `ConnectionString()` preserve the URL’s `sslmode` while still adding cert paths
- REFACTOR:
  - delete `DBTLSMode` and any DTOs/tests that only existed to carry the duplicate setting

### Slice 5: Flatten Remaining Verify-Service Config Noise

- RED:
  - add a failing test that proves nested `raw_table_output.enabled` is gone or replaced by one direct boolean
  - if execution shows the feature should no longer be configurable, add the failing assertion that the endpoint is always available and the old toggle is rejected as obsolete config
- GREEN:
  - implement the smallest honest flattening/removal that matches the runtime behavior
- REFACTOR:
  - delete any no-longer-needed config wrappers instead of preserving translation code

### Slice 6: Update CLI, README, And Verify Image Contracts

- RED:
  - add the next failing CLI/README/image-harness assertions that encode:
    - the smaller YAML shape
    - honest supported listener modes
    - removal of duplicate DB `verify-*` knobs
- GREEN:
  - update help output, README examples, harness config generation, and fixture files
- REFACTOR:
  - keep operator-surface wording owned by the existing CLI/README support boundaries instead of scattering raw string checks

### Slice 7: Final Lanes And Boundary Pass

- RED:
  - after behavior slices are green, run:
    - `make check`
    - `make lint`
    - `make test`
  - do not run `make test-long` unless execution ends up changing long-lane selection or the task proves the long lane is required
- GREEN:
  - continue until all required default lanes pass cleanly
- REFACTOR:
  - do one final `improve-code-boundaries` pass so listener-mode policy, DB connection policy, and operator-surface wording each live at one honest boundary

## TDD Guardrails For Execution

- One failing behavioral slice at a time.
- Do not add tests after the implementation for the same behavior.
- Do not preserve old nested YAML fields behind hidden compatibility shims.
- Do not swallow config or startup errors.
- Do not keep both URL `sslmode` and YAML `tls.mode`; choose one source of truth.
- Do not keep HTTP blocked through a policy check disguised as validation.
- If the first RED slice shows the capability-based listener shape is wrong, switch this plan back to `TO BE VERIFIED` and stop immediately instead of forcing the wrong contract through.

## Final Verification For The Execution Turn

- [ ] Red/green TDD covers config parsing and startup for plain HTTP, HTTPS without mTLS, and HTTPS with mTLS
- [ ] The service does not reject startup solely because the operator chose HTTP or disabled mTLS
- [ ] Source and destination TLS verification behavior has one source of truth each
- [ ] Needlessly nested config booleans such as inner `enabled` toggles are removed or flattened where a direct contract is clearer
- [ ] CLI help, README examples, and fixture configs document the supported listener and DB TLS modes without implying fake requirements
- [ ] `make check`
- [ ] `make lint`
- [ ] `make test`
- [ ] Do not run `make test-long` unless the task explicitly requires it or long-lane selection changes
- [ ] Final `improve-code-boundaries` pass confirms the config/runtime contract got smaller and cleaner
- [ ] Update the task file checkboxes and set `<passes>true</passes>` only after every required lane passes

Plan path: `.ralph/tasks/story-27-verify-operator-ux-reset/01-task-reset-verify-service-config-to-operator-chosen-security-and-remove-redundant-tls-knobs_plans/2026-04-20-verify-service-security-contract-reset-plan.md`

NOW EXECUTE
