# Cockroach Migrate Tool

This repository contains the first Rust workspace for the CockroachDB-to-PostgreSQL migration runner.

## Workspace Layout

- `crates/runner`: destination-side runtime for validated config loading, PostgreSQL access wiring, webhook runtime wiring, and reconcile runtime wiring.
- `crates/source-bootstrap`: source-side CLI for creating changefeed bootstrap plans from typed YAML config.

## Command Contract

- `make check`: run the workspace lint gate.
- `make lint`: same as `make check`.
- `make test`: run the default workspace test suite.
- `make test-long`: run the ignored long-test lane.

Raw Cargo commands remain available when you want a narrower loop:

- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

## Dependency Policy

The foundation of this workspace is intentionally opinionated.

- PostgreSQL access uses `sqlx`.
- Application error boundaries use `thiserror`.
- CLI parsing uses `clap`.
- YAML configuration uses `serde` and `serde_yaml`.
- HTTP runtime code uses `axum`.
- TLS and HTTP must use established crates. Hand-rolled protocol, config, or error layers are not allowed.

If a future change needs a new dependency, it must establish a real boundary or behavior in the same story. Dependency-only tasks and throwaway wrappers are out of scope for this codebase.
