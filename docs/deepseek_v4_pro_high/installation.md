# Installation

There are several ways to obtain and run the migration tool.

## Docker Images (Recommended)

Pre-built multi-arch images (linux/amd64 + linux/arm64) are published to two registries:

### Quay.io

| Component | Image |
|---|---|
| runner | `quay.io/<org>/runner:<tag>` |
| setup-sql | `quay.io/<org>/setup-sql:<tag>` |
| verify | `quay.io/<org>/verify:<tag>` |

### GitHub Container Registry (GHCR)

| Component | Image |
|---|---|
| runner | `ghcr.io/<org>/cockroach-migrate-runner:<tag>` |
| setup-sql | `ghcr.io/<org>/cockroach-migrate-setup-sql:<tag>` |
| verify | `ghcr.io/<org>/cockroach-migrate-verify:<tag>` |

Available tags:
- `latest` — Most recent promoted release
- `<sha>-amd64`, `<sha>-arm64` — Platform-specific builds from master
- Semantic version tags (e.g., `v1.2.3`) when explicitly promoted

Pull an image:

```bash
docker pull quay.io/<org>/runner:latest
```

All images are built from `scratch` (empty base layer) — they contain only the statically-linked binary. This keeps images small (<20MB) and reduces the attack surface.

## Building from Source

### Prerequisites

- **Rust** 1.93+ (2024 edition) with `musl-tools` for static linking
- **Go** 1.26+ (required only for the verify component)
- **Cargo Chef** — install separately or let the Dockerfiles handle it

### Clone the Repository

```bash
git clone <repository-url>
cd cockroach_migrate_tool
```

### Build All Rust Components

```bash
cargo build --release -p runner -p setup-sql
```

The binaries will be at:
- `target/release/runner`
- `target/release/setup-sql`

### Build with Static musl Linking (for scratch Docker images)

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl -p runner -p setup-sql
```

### Build the Verify Component (Go)

```bash
cd cockroachdb_molt/molt
CGO_ENABLED=0 go build -o molt
```

### Run Tests

```bash
# Rust workspace tests
cargo test --workspace

# Verify service Go tests
cd cockroachdb_molt/molt && go test ./cmd/verifyservice -count=1

# Long-running integration tests (single-threaded)
cargo test --workspace -- --ignored --test-threads=1

# Linting
cargo clippy --workspace --all-targets -- -D warnings
```

## Building Docker Images Locally

### Runner

```bash
docker build -t runner:local -f Dockerfile .
```

### Setup SQL

```bash
docker build -t setup-sql:local -f crates/setup-sql/Dockerfile .
```

### Verify

```bash
docker build -t verify:local -f cockroachdb_molt/molt/Dockerfile ./cockroachdb_molt/molt
```

## Docker Compose Artifacts

Pre-built Docker Compose snippets are available in `artifacts/compose/`:

| File | Purpose |
|---|---|
| `runner.compose.yml` | Service definition for the runner, mounted config and TLS certs |
| `setup-sql.compose.yml` | Service definition for setup-sql, network_mode: none, mounts configs |
| `verify.compose.yml` | Service definition for the verify service with TLS certs |

These are designed as compose *snippets* to be merged into your existing infrastructure. They reference environment variables for image tags (`RUNNER_IMAGE`, `SETUP_SQL_IMAGE`, `VERIFY_IMAGE`) and ports (`RUNNER_HTTPS_PORT`, `VERIFY_HTTPS_PORT`).

## Rust Workspace Structure

```
crates/
├── ingest-contract/     # Shared URL path contract
├── operator-log/        # Structured logging (text + JSON)
├── runner/              # Main runtime daemon
└── setup-sql/           # SQL emission CLI

cockroachdb_molt/molt/   # Go verify service (standalone module)
```

The workspace uses Rust edition 2024 with resolver 2. Key dependencies:

| Dependency | Purpose |
|---|---|
| `axum` 0.8 | HTTP server framework (runner webhook) |
| `clap` 4.5 | CLI argument parsing |
| `sqlx` 0.8 | Async PostgreSQL driver with compile-time query checking |
| `rustls` 0.23 | TLS implementation (server + client) |
| `serde` / `serde_yaml` | YAML config deserialization |
| `tokio` 1.48 | Async runtime |
