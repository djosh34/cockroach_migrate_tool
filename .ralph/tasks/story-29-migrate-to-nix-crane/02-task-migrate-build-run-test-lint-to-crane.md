## Task: Migrate Build Run Test And Lint To Crane <status>done</status> <passes>true</passes>

<description>
**Goal:** Replace the current local build, run, test, check, and lint workflow with a fully reproducible Nix flake built around crane. The higher order goal is to make local development and CI share one Nix-native build graph where Rust artifacts are reused correctly and only the code that actually changed is rebuilt.

In scope:
- add or update the repository Nix flake and lock file needed for a crane-based Rust build
- use crane properly so tests reuse build artifacts instead of rebuilding from scratch
- split dependency and source builds so code-only edits recompile only the changed source layer where crane supports that behavior
- provide Nix-native equivalents for build, run, check, test, long-test where applicable, formatting, and linting
- fully replace `make lint`, `make test`, and related Make-based developer entrypoints with Nix-based commands or remove the Make dependency entirely
- ensure all current binaries and test suites remain reachable through Nix
- ensure Nix commands fail loudly on any error and do not mask underlying Rust, lint, or test failures
- update repository documentation or task notes so future agents know the canonical Nix commands

Out of scope:
- Docker image generation through Nix
- GitHub Actions migration
- support for machines without native Nix
- preserving old Make behavior for backwards compatibility

Decisions already made:
- the project is greenfield and has no backwards compatibility requirement
- old Make-centric local workflows should be fully replaced rather than kept in parallel
- the setup must use crane and must use crane artifact reuse advantages, not merely wrap Cargo commands in Nix
- the resulting build must be reproducible and usable locally

</description>


<acceptance_criteria>
- [x] A Nix flake provides the canonical local build, run, check, test, long-test where applicable, format, and lint commands.
- [x] crane is used as the Rust build foundation and is configured to separate dependency artifacts from source artifacts where practical.
- [x] Manual verification: a clean Nix build succeeds from the project workspace.
- [x] Manual verification: Nix-based tests succeed and reuse the build artifacts produced by the Nix build where crane supports reuse.
- [x] Manual verification: Nix-based lint/check commands fail on real lint/check failures and pass cleanly on the final tree.
- [x] Manual verification: after a code-only change, the Nix/crane build graph avoids rebuilding unchanged dependencies; task notes include the command/output evidence used to verify this.
- [x] Make-based build/test/lint entrypoints are removed or replaced so contributors cannot accidentally use a non-Nix path as the canonical workflow.
- [x] Documentation or task notes identify the new canonical local commands and explicitly state that the old Make workflow is gone.
</acceptance_criteria>

<plan>.ralph/tasks/story-29-migrate-to-nix-crane/02-task-migrate-build-run-test-lint-to-crane_plans/2026-04-28-migrate-build-run-test-lint-to-crane-plan.md</plan>

<execution_notes>
- Canonical local Nix commands:
  - `nix build .#runner`
  - `nix build .#verify-service`
  - `nix run .#runner -- --help`
  - `nix run .#verify-service -- --help`
  - `nix run .#check`
  - `nix run .#lint`
  - `nix run .#test`
  - `nix run .#fmt`
  - `nix run .#test-long`
  - `nix develop`
- Flake boundary and public surfaces:
  - Added `flake.nix` and `flake.lock`.
  - Rust build/test/lint/fmt use crane-backed derivations.
  - Go `verify-service` uses `buildGoModule` with `modRoot = cockroachdb_molt/molt` so repo-level OpenAPI and fixture contracts stay visible to tests.
  - `Makefile` is now a thin compatibility shim that delegates directly to the Nix public commands above.
  - Contributor docs were rewritten to state that the old Make workflow is gone as a canonical interface.
- Default test-lane boundary cleanup:
  - Reclassified Docker/image/fallback contracts onto the ignored long lane so the default `test` surface matches the repo rule that long/e2e validation is story-end only.
  - Files moved to the long lane by `#[ignore = "long lane"]`:
    - `crates/runner/tests/default_bootstrap_long_lane.rs` for the two previously unignored verify-image e2e cases
    - `crates/runner/tests/image_contract.rs`
    - `crates/runner/tests/verify_image_contract.rs`
    - `crates/runner/tests/novice_registry_only_contract.rs`
- Dependency-reuse evidence:
  - Baseline commands:
    - `nix build .#cargo-artifacts --print-out-paths` -> `/nix/store/jmhd23yc9inhj3c6yjgs93xc9zk2giqi-runner-deps-deps-0.1.0`
    - `nix build .#runner --print-out-paths` -> `/nix/store/flq8j3bqrfjhngkraa968as18rlh5398-runner-0.1.0`
  - Temporary code-only probe:
    - added then removed a one-line comment in `crates/runner/src/main.rs`
  - Rebuild after the source-only edit:
    - `nix build .#cargo-artifacts --print-out-paths` -> `/nix/store/jmhd23yc9inhj3c6yjgs93xc9zk2giqi-runner-deps-deps-0.1.0`
    - `nix build .#runner --print-out-paths` -> `/nix/store/mplgrls0vz9bqsmq4zhdwn1bk3z6d12d-runner-0.1.0`
  - Result:
    - dependency artifact path stayed unchanged
    - source package path changed
    - this demonstrates the crane dependency/source split is working for code-only Rust edits
- Nix-native validation executed during task work:
  - `nix build .#runner`
  - `nix build .#verify-service`
  - `nix run .#lint`
  - `nix run .#test`
  - `nix run .#fmt`
  - `nix run .#runner -- --help`
  - `nix run .#verify-service -- --help`
  - `nix eval --raw .#apps.x86_64-linux.test-long.program`
- Required final repo gates:
  - `make check` -> passed
  - `make lint` -> passed
  - `make test` -> passed
- Improve-code-boundaries review:
  - The old orchestration boundary in `Makefile` was flattened into the flake.
  - Rust package/test/lint logic and Go verify-service logic now live behind explicit Nix outputs instead of being split across ad hoc host tools.
  - Default per-task validation no longer accidentally includes Docker/image/fallback e2e contracts; those remain reachable through the long lane where they belong.
</execution_notes>
