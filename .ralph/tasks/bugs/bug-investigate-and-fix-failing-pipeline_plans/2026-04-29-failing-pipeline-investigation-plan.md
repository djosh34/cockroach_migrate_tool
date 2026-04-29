# Plan: Fix The Hosted `nix flake check` Pipeline Failure

## References

- Task:
  - `.ralph/tasks/bugs/bug-investigate-and-fix-failing-pipeline.md`
- Authoritative hosted failure:
  - workflow run `25101383986`
  - workflow `Publish Images`
  - job `73551223578` named `nix flake check`
  - failing step `Run nix flake check`
  - exact hosted command: `nix flake check --print-build-logs --show-trace`
- Relevant pipeline surfaces:
  - `.github/workflows/publish-images.yml`
  - `flake.nix`
- Relevant failing code path:
  - `crates/runner/tests/support/verify_service_harness.rs`
  - `crates/runner/tests/support/e2e_harness.rs`
  - `crates/runner/tests/support/multi_mapping_harness.rs`
  - `cockroachdb_molt/molt/verifyservice/config.go`
  - `cockroachdb_molt/molt/verifyservice/resolved_config.go`
- Required skills:
  - `tdd`
  - `improve-code-boundaries`
  - `github-api-auth-wrapper`

## Authoritative Failure Summary

- Hosted evidence came from GitHub Actions logs, not local guessing.
- The latest failing hosted run at `2026-04-29T09:32:35Z` is:
  - `https://github.com/djosh34/cockroach_migrate_tool/actions/runs/25101383986`
- The exact failing job is:
  - `nix flake check`
- The exact failing command is:
  - `nix flake check --print-build-logs --show-trace`
- The failing behavior inside that command is not workflow bootstrap, Cachix, or image publication.
  - It is the repo-owned long-lane verify path executed by flake checks.
- The concrete failure text from the hosted logs is:
  - `verify-service config is invalid: yaml: unmarshal errors:`
  - `line 2: field url not found in type verifyservice.DatabaseConfig`
  - `line 8: field url not found in type verifyservice.DatabaseConfig`
- The hosted failing tests are the ignored long-lane runner tests that start the Go `molt verify-service` process.
  - They fail before the verify runtime becomes ready because the generated YAML still uses the removed `url:` field.

## Planning Assumptions

- This is ready for execution next turn.
  - The root cause has been narrowed to a real repo regression with authoritative hosted evidence.
- This is not a workflow-only bug.
  - The pipeline merely exposed a code/support mismatch that local default gates do not exercise.
- This is also not a Rust application-behavior bug in the product runtime.
  - The break is in Rust test support generating stale verify-service config for the Go verify binary.
- Because of that, the TDD shape for execution is:
  - use the existing failing hosted-equivalent long-lane command as RED
  - fix one harness/config slice at a time
  - avoid inventing irrelevant new Rust unit tests for YAML strings
- If execution shows the Go verify-service contract itself changed beyond `url` removal, and the new honest boundary is not the planned one-database mapping shape below, switch this file back to `TO BE VERIFIED` and stop immediately.

## Boundary Problem To Flatten

- The live boundary smell is in `crates/runner/tests/support/verify_service_harness.rs`.
  - It hand-renders verify-service YAML from raw URL strings.
  - That duplicates connection-shape knowledge outside the real verify-service config model.
- The Go verify-service config no longer accepts a flat `url` field in `DatabaseConfig`.
  - It now owns a typed database config plus `verify.databases` mappings.
- The old Rust harness is therefore a stale compatibility layer.
  - It should be removed, not patched with another stringly alias.
- This is the `improve-code-boundaries` target for the task:
  - move the Rust harness from ad hoc URL-to-YAML rendering
  - to one typed verify-service config rendering seam that mirrors the real Go config contract
  - and centralize connection-string decomposition in one place

## Current State Summary

- `verify_service_harness.rs` currently writes:
  - `verify.source.url: ...`
  - `verify.destination.url: ...`
- `cockroachdb_molt/molt/verifyservice/config.go` rejects those fields through known-field decoding.
- `cockroachdb_molt/molt/verifyservice/config.go` also requires:
  - `verify.databases` to contain at least one mapping
- `cockroachdb_molt/molt/verifyservice/resolved_config.go` resolves the effective connection from:
  - top-level default `verify.source` and `verify.destination`
  - plus one or more `verify.databases[*]` mappings
- The existing Rust harness inputs are currently:
  - `source_url`
  - `destination_url`
  - include patterns
  - expected tables
- That means the likely honest execution shape is:
  - keep the public Rust test callers passing URLs for now
  - decompose those URLs once inside the harness
  - render the new typed verify-service YAML with one database mapping instead of flat `url` fields

## Proposed Interface Shape

- Introduce one narrow internal Rust type in `verify_service_harness.rs` for the rendered verify-service config.
  - Preferred split:
    - `VerifyServiceConfig`
    - `VerifyDatabaseDefaults`
    - `VerifyDatabaseMapping`
    - `VerifyTlsConfig`
- Introduce one internal parsed connection type for the harness only.
  - Preferred shape:
    - `ParsedPostgresUrl { host, port, database, username, password, sslmode, tls_paths }`
- Keep current outer test call sites stable initially.
  - `VerifyServiceRun` may continue accepting `source_url` and `destination_url` as inputs.
  - The harness should parse those once and own the typed YAML rendering from then on.
- Do not add backwards compatibility in the Go config.
  - No `url` alias
  - no legacy parser path
  - no silent translation in Go

## Public Contract To Preserve

- The hosted failing path must stop failing:
  - `nix flake check --print-build-logs --show-trace`
- The relevant long-lane verify-service runner tests must pass again.
- Default local gates must still pass:
  - `make check`
  - `make lint`
  - `make test`
- Because this bug is in the hosted long-lane path, execution must also verify the affected long path directly.
  - Prefer a focused long-lane build during red-green work.
  - Then run the full hosted-equivalent `nix flake check --print-build-logs --show-trace` before closing the task.

## Red-Green Execution Plan

### Slice 1: Reproduce The Hosted Failure Locally

- [ ] RED:
  - reproduce the failing hosted class locally with the smallest honest command, preferably:
    - `nix build .#checks.x86_64-linux.runner-crate-nextest-long --print-build-logs --show-trace`
  - if that does not reproduce cleanly, fall back to:
    - `nix flake check --print-build-logs --show-trace`
  - confirm the failure still reports `field url not found in type verifyservice.DatabaseConfig`
- [ ] GREEN:
  - no code changes in this slice
  - only lock the exact local repro command that matches hosted evidence
- [ ] REFACTOR:
  - none

### Slice 2: Replace Stringly Verify-Service YAML With Typed Harness Rendering

- [ ] RED:
  - keep the repro command failing while isolating the stale YAML writer in `verify_service_harness.rs`
- [ ] GREEN:
  - add internal typed config/rendering helpers in `verify_service_harness.rs`
  - stop writing `verify.source.url` and `verify.destination.url`
  - emit:
    - typed top-level source defaults
    - typed top-level destination defaults
    - a one-entry `verify.databases` list
  - ensure rendered fields match the Go config contract exactly
- [ ] REFACTOR:
  - keep all verify-service YAML ownership in one place
  - remove duplicated string assembly where possible

### Slice 3: Centralize URL Decomposition Instead Of Sprinkling Parsing Logic

- [ ] RED:
  - if Slice 2 needs ad hoc parsing in multiple spots, stop and consolidate before expanding edits
- [ ] GREEN:
  - parse `source_url` and `destination_url` once into one internal harness type
  - convert URL query items such as `sslmode`, `sslrootcert`, `sslcert`, and `sslkey` into the nested typed TLS shape expected by verify-service
  - fail loudly on unsupported or malformed URLs instead of silently dropping fields
- [ ] REFACTOR:
  - keep parsing and config rendering separate:
    - one step decomposes URL input
    - one step renders verify-service config

### Slice 4: Flatten The Remaining Boundary Smell

- [ ] RED:
  - inspect `e2e_harness.rs` and `multi_mapping_harness.rs` after the first green slice
  - if they duplicate verify-service config knowledge beyond raw run inputs, treat that as remaining mud
- [ ] GREEN:
  - keep only one seam responsible for verify-service config shape
  - move any obvious duplicate conversion or YAML-shape assumptions behind that seam
- [ ] REFACTOR:
  - remove dead helpers or fields made obsolete by the typed config rendering path

### Slice 5: Verify The Hosted Path End To End

- [ ] Run the focused affected long-lane command used for RED until it passes.
- [ ] Run `make check`.
- [ ] Run `make lint`.
- [ ] Run `make test`.
- [ ] Run the full hosted-equivalent command:
  - `nix flake check --print-build-logs --show-trace`
- [ ] Do one final `improve-code-boundaries` sweep:
  - no stale `url:` rendering remains in verify-service harness code
  - no second config schema is kept alive in Rust support
  - parsing errors are explicit and not swallowed

## Execution Guardrails

- Do not patch the Go verify-service to accept legacy `url`.
- Do not add fake Rust tests that only assert YAML strings.
- Do not suppress URL parsing or config rendering errors.
- Do not skip the affected long-lane validation once the code is fixed.
- Do not run `cargo`; use Nix-backed commands only.
- Do not “fix” the pipeline by merely removing the long-lane path from `nix flake check` unless execution proves the long lane itself is wrongly wired and not the broken code path.

## Expected Outcome

- The hosted pipeline failure is explained by one concrete repo regression, with exact run/job/command evidence already captured.
- Rust test support stops generating a stale verify-service config schema.
- The verify-service harness owns one honest typed config boundary instead of a deprecated URL-to-YAML shortcut.
- The affected long-lane path passes locally again, and the full hosted-equivalent flake check passes before the task is closed.

Plan path: `.ralph/tasks/bugs/bug-investigate-and-fix-failing-pipeline_plans/2026-04-29-failing-pipeline-investigation-plan.md`

NOW EXECUTE
