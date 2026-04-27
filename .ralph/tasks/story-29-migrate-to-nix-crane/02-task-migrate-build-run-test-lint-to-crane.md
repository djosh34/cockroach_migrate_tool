## Task: Migrate Build Run Test And Lint To Crane <status>not_started</status> <passes>false</passes>

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
- [ ] A Nix flake provides the canonical local build, run, check, test, long-test where applicable, format, and lint commands.
- [ ] crane is used as the Rust build foundation and is configured to separate dependency artifacts from source artifacts where practical.
- [ ] Manual verification: a clean Nix build succeeds from the project workspace.
- [ ] Manual verification: Nix-based tests succeed and reuse the build artifacts produced by the Nix build where crane supports reuse.
- [ ] Manual verification: Nix-based lint/check commands fail on real lint/check failures and pass cleanly on the final tree.
- [ ] Manual verification: after a code-only change, the Nix/crane build graph avoids rebuilding unchanged dependencies; task notes include the command/output evidence used to verify this.
- [ ] Make-based build/test/lint entrypoints are removed or replaced so contributors cannot accidentally use a non-Nix path as the canonical workflow.
- [ ] Documentation or task notes identify the new canonical local commands and explicitly state that the old Make workflow is gone.
</acceptance_criteria>
