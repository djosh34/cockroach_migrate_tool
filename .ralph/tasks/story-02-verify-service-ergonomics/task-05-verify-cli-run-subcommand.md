## Task: Add explicit `run` subcommand to verify service for CLI consistency <status>not_started</status> <passes>false</passes>

<description>
Must use tdd skill to complete

**Goal:** Make the verify service CLI consistent with `runner` and `setup-sql` by adding an explicit `run` subcommand. Currently the verify service entrypoint in `cockroachdb_molt/molt/cmd/verifyservice/verifyservice.go` accepts bare `--config` without a subcommand (e.g., `verify-service --config ...`), while runner requires `runner run --config ...`. This inconsistency trips users who expect the same CLI pattern across all three binaries.

**In scope:**
- Modify the verify service cobra command setup to add a `run` subcommand that accepts `--config` and `--log-format`.
- Keep the existing bare `--config` invocation working for backward compatibility (or at minimum, provide a clear migration path).
- Update the README verify quick start to use `verify-service run --config ...`.
- Update Docker Compose examples to use the explicit subcommand.
- Update Go tests in `cockroachdb_molt/molt/cmd/verifyservice/verifyservice_test.go`.
- Update the operator CLI surface contract test in `crates/runner/tests/operator_cli_surface_contract.rs`.

**Out of scope:**
- Removing bare `--config` support (if doing so, must be a breaking change with clear notice).
- Adding other subcommands beyond `run`.
- Changing runner or setup-sql CLI structure.

**End result:**
Users can run:
```bash
verify-service run --config /config/verify-service.yml --log-format json
```
matching the pattern of:
```bash
runner run --config /config/runner.yml --log-format json
```
</description>

<acceptance_criteria>
- [ ] `verify-service run --config <path>` starts the service successfully
- [ ] `verify-service run --config <path> --log-format json` works
- [ ] README and Compose examples use the `run` subcommand
- [ ] Operator CLI surface contract test reflects the new subcommand
- [ ] Existing Go tests for verify service still pass
- [ ] `make check` — passes cleanly
- [ ] `make test` — passes cleanly (default suite)
- [ ] `make lint` — passes cleanly
</acceptance_criteria>
