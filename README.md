# Cockroach Migrate Tool

This repository contains the first Rust workspace for the CockroachDB-to-PostgreSQL migration runner.

## Workspace Layout

- `crates/runner`: destination-side runtime for validated config loading, PostgreSQL access wiring, webhook runtime wiring, and reconcile runtime wiring.
- `crates/source-bootstrap`: source-side CLI for creating changefeed bootstrap plans from typed YAML config.
- `Dockerfile`: single-binary destination image for the `runner` runtime.

## Docker Quick Start

The destination runtime is one container that starts the `runner` binary directly. There is no wrapper shell script in the user path.

1. Create a config directory with one runner config file and the TLS material it references.

```yaml
# config/runner.yml
postgres:
  host: pg.example.internal
  port: 5432
  database: migration_db
  user: migration_user
  password: runner-secret
webhook:
  bind_addr: 0.0.0.0:8443
  tls_cert_path: /config/certs/server.crt
  tls_key_path: /config/certs/server.key
reconcile:
  interval_secs: 30
```

2. Build the destination image directly from the repository root:

```bash
docker build -t cockroach-migrate-runner .
```

3. Validate the mounted config directly through the image entrypoint:

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  cockroach-migrate-runner \
  validate-config --config /config/runner.yml
```

4. Start the destination runtime directly through the image entrypoint:

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  cockroach-migrate-runner \
  run --config /config/runner.yml
```

The mounted `/config` directory is the only Docker-specific convention. The same `runner validate-config --config <path>` and `runner run --config <path>` interface remains the public contract on the host and in the container.

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
