## Task: Reset verify-service config to operator-chosen security and remove redundant TLS knobs <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete


**Goal:** Rework the verify-service config and startup contract so the operator chooses the security posture instead of the product blocking “insecure” deployments by policy. The higher order goal is to make the verify image usable in real environments where users may want plain HTTP, HTTPS without mTLS, or HTTPS with mTLS, while still making the tradeoffs explicit and keeping the config surface small and coherent.

Current product gap from the 2026-04-20 user review:
- the verify runner is not user-friendly
- the service currently dictates what is and is not allowed instead of letting the operator choose
- the config and validation surface is too deeply nested
- the current contract appears to require duplicated TLS verification choices such as specifying `verify-full` or `verify-ca` more than once
- previous backlog direction leaned toward forbidding or security-shaming insecure listener modes, but product direction has now changed

In scope:
- allow the verify HTTP service to run in operator-chosen modes including plain HTTP, HTTPS without mTLS, and HTTPS with mTLS
- keep insecure modes explicit in config and logs, but do not reject startup solely because the operator chose HTTP or no mTLS
- remove redundant TLS verification knobs so the same source or destination verification behavior is not specified twice in different config layers
- simplify the verify-service config shape by collapsing needless nesting and removing “enabled inside nested structs” style toggles where one direct top-level contract is enough
- make the remaining config fields reflect only behavior that is actually consumed by the verify runtime
- update CLI help, README examples, fixture configs, and contract tests so the supported modes are obvious

Out of scope:
- redesigning the verify algorithm itself
- changing the single-active-job model
- returning richer mismatch payloads beyond what is required to keep config and startup errors coherent

Decisions already made:
- product direction now explicitly allows operator-chosen insecure modes; the software must not hard-block HTTP or lack of mTLS just because the mode is less secure
- the software may still explain tradeoffs, but explanation must not masquerade as a ban
- duplicated verify-mode choices such as `verify-full` / `verify-ca` in multiple config layers should be collapsed to one source of truth
- this is a greenfield project with no backwards-compatibility requirement, so dead config and awkward nesting should be removed rather than preserved
- this task should revisit earlier verify-service security-direction assumptions from `story-18-verify-http-image/04-task-add-verify-service-config-for-source-destination-tls-and-mode-selection.md` and related verify-security bug tasks where those assumptions conflict with the current product direction

Relevant files and boundaries:
- `cockroachdb_molt/molt/verifyservice/config.go`
- `cockroachdb_molt/molt/verifyservice/config_test.go`
- `cockroachdb_molt/molt/verifyservice/runtime.go`
- `cockroachdb_molt/molt/verifyservice/runtime_test.go`
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go`
- `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`
- `README.md`
- `crates/runner/tests/verify_image_contract.rs`
- `crates/runner/tests/readme_operator_surface_contract.rs`
- `crates/runner/tests/support/verify_image_harness.rs`

</description>


<acceptance_criteria>
- [ ] Red/green TDD covers config parsing and startup for plain HTTP, HTTPS without mTLS, and HTTPS with mTLS
- [ ] The service does not reject startup solely because the operator chose HTTP or disabled mTLS
- [ ] Source and destination TLS verification behavior has one source of truth each; duplicated `verify-full` / `verify-ca` configuration is removed
- [ ] Needlessly nested config booleans such as inner `enabled` toggles are removed or flattened where a direct contract is clearer
- [ ] CLI help, README examples, and fixture configs document the supported listener and DB TLS modes without implying fake requirements
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite; excludes only ultra-long tests moved to `make test-long`)
- [ ] `make lint` — passes cleanly
- [ ] If this task impacts ultra-long tests (or their selection): `make test-long` — passes cleanly (ultra-long-only)
</acceptance_criteria>
