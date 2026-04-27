## Bug: Runner Image Contract Tests Exhaust Docker Disk <status>not_started</status> <passes>false</passes> <priority>medium</priority>

<description>
`make test` failed during `crates/runner/tests/image_contract.rs` while building the runner Docker image. The Docker build failed in the `cargo install cargo-chef --locked` layer because unpacking `petgraph v0.8.3` returned `No space left on device (os error 28)`.

Observed failing tests:

- `runner_image_builds_from_the_root_runtime_slice`
- `runner_image_exposes_a_direct_runtime_only_entrypoint`
- `runner_image_runtime_filesystem_contains_only_the_runner_payload`
- `runner_image_validate_config_supports_json_operator_logs`
- `runner_image_help_surface_stays_runtime_only`

The failure was detected while validating docs-only changes with `make test`.
</description>

<manual_verification_required>
This is an environment/Docker build capacity failure, not a product behavior failure. Fix by freeing or increasing Docker build storage, improving image build cache behavior, or otherwise making the runner image contract build reliable in the default test environment. TDD is not required for this non-code/environment task.
</manual_verification_required>

<acceptance_criteria>
- [ ] Docker has enough build storage for the runner image contract tests.
- [ ] `cargo test -p runner --test image_contract` passes cleanly.
- [ ] `make test` passes cleanly.
- [ ] `make lint` passes cleanly.
</acceptance_criteria>
