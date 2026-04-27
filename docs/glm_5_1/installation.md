# Installation

## Pre-built Container Images

The recommended way to run the CockroachDB Migration Tool is via pre-built Docker images. The project publishes multi-architecture images (amd64 and arm64) for each component.

Three container images are available:

| Component | Binary | Image |
|-----------|--------|-------|
| Runner | `runner` | `quay.io/cockroach/runner` |
| Setup SQL | `setup-sql` | `quay.io/cockroach/setup-sql` |
| Verify | `molt` | `quay.io/cockroach/verify` |

Each image uses a `FROM scratch` runtime with a single static binary and no shell — minimal attack surface, no OS packages.

```bash
docker pull quay.io/cockroach/runner:latest
docker pull quay.io/cockroach/setup-sql:latest
docker pull quay.io/cockroach/verify:latest
```

### Running the Images

Runner:

```bash
docker run --rm \
  -v ./config:/config:ro \
  quay.io/cockroach/runner:latest \
  run --log-format json --config /config/runner.yml
```

Setup SQL (emits SQL to stdout, no network needed):

```bash
docker run --rm \
  --network none \
  -v ./config:/config:ro \
  quay.io/cockroach/setup-sql:latest \
  emit-cockroach-sql --log-format json --config /config/cockroach-setup.yml
```

Verify service:

```bash
docker run --rm \
  -p 9443:8080 \
  -v ./config:/config:ro \
  quay.io/cockroach/verify:latest \
  verify-service run --log-format json --config /config/verify-service.yml
```

## Building from Source

### Prerequisites

- **Rust** 1.93+ (for runner and setup-sql)
- **Go** 1.26+ (for the verify service)
- **musl-tools** (for static Linux builds)
- **Docker** with BuildKit (for container builds)

### Clone the Repository

```bash
git clone <repository-url> cockroach_migrate_tool
cd cockroach_migrate_tool
```

### Build Rust Binaries

The Rust workspace contains three crates:

```bash
# Build all crates
cargo build --release --workspace

# Build individual binaries
cargo build --release -p runner --bin runner
cargo build --release -p setup-sql --bin setup-sql
```

The binaries appear at `target/release/runner` and `target/release/setup-sql`.

For a fully static Linux binary (musl target):

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl -p runner --bin runner
```

### Build the Verify Binary

```bash
cd cockroachdb_molt/molt
go build -trimpath -ldflags="-s -w" -o molt .
cd ../..
```

### Run Tests

```bash
# Fast tests (unit + integration, Rust)
make test

# Long-running tests
make test-long

# Lint
make lint
```

## Building Container Images

Each component has a multi-stage Dockerfile that uses `cargo-chef` for dependency caching:

```bash
# Runner
docker build -t runner -f Dockerfile .

# Setup SQL
docker build -t setup-sql -f crates/setup-sql/Dockerfile .

# Verify
docker build -t verify -f cockroachdb_molt/molt/Dockerfile ./cockroachdb_molt/molt
```

Build args:
- `TARGETARCH` — `amd64` or `arm64` (for musl cross-compilation target selection)

The Dockerfiles use a three-stage pattern:
1. **chef** — installs `cargo-chef` and cross-compilation toolchains
2. **planner** — computes the dependency recipe (`cargo chef prepare`)
3. **builder** — compiles the binary (`cargo chef cook` + `cargo build`)
4. **runtime** — `FROM scratch`, copies only the compiled binary

## Docker Compose Deployment

The project includes Compose files under `artifacts/compose/` for production-style deployment:

```bash
# Set image variables
export RUNNER_IMAGE=quay.io/cockroach/runner:latest
export SETUP_SQL_IMAGE=quay.io/cockroach/setup-sql:latest
export VERIFY_IMAGE=quay.io/cockroach/verify:latest

# Generate setup SQL (one-time)
docker compose -f artifacts/compose/setup-sql.compose.yml up

# Start the runner (long-running)
docker compose -f artifacts/compose/runner.compose.yml up -d

# Start the verify service (long-running)
docker compose -f artifacts/compose/verify.compose.yml up -d
```

Each Compose file mounts configuration and certificates via Docker configs. You need to provide:

- **Runner**: `runner.yml`, server certificate, server key
- **Setup SQL**: `cockroach-setup.yml`, `postgres-grants.yml`, CA certificate
- **Verify**: `verify-service.yml`, source/destination TLS certs, server TLS certs (and optional client CA for mTLS)