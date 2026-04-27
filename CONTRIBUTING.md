# Contributing

`README.md` is the operator-facing contract. This document owns contributor workflow, repository structure, and local validation commands.

## Workspace Layout

- `crates/runner`: destination-side runtime for validated config loading, PostgreSQL access wiring, webhook runtime wiring, and reconcile runtime wiring.
- `cockroachdb_molt/molt`: Go module that exposes the `molt verify-service` runtime and verify-service test lane.
- `scripts`: SQL generator scripts for CockroachDB source setup and PostgreSQL destination grants.

## Command Contract

Canonical local workflow is Nix-native. Do not use the old Make workflow as a contributor interface.

- `nix build .#runner`: build the Rust `runner` binary through crane.
- `nix build .#verify-service`: build the wrapped `molt verify-service` surface.
- `nix run .#runner -- --help`: inspect the `runner` CLI surface.
- `nix run .#verify-service -- --help`: inspect the `verify-service` CLI surface.
- `nix run .#check`: run the clippy gate with `-D warnings`.
- `nix run .#lint`: alias of `nix run .#check`.
- `nix run .#test`: run the default Rust and Go test lanes.
- `nix run .#fmt`: run the Rust formatting check.
- `nix flake check`: run the normal flake check set.
- `nix run .#test-long`: run the ignored long/e2e lane. This is story-end validation only, not the default per-task lane.
- `nix develop`: enter the matching Rust/Go/script toolchain shell.

`make check`, `make lint`, `make test`, and `make test-long` remain only as thin compatibility shims that delegate straight to the Nix commands above. They are not the source of truth.
