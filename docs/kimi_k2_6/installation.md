# Installation

This project is written in Rust and distributed as source code and container images. Choose the method that fits your environment.

## Requirements

- **Rust toolchain** `1.93` or later (if building from source)
- **Docker** `20.10+` or any OCI-compatible runtime (if using pre-built images)
- A **CockroachDB** source cluster (`v22.2+` recommended for enriched webhook changefeeds)
- A **PostgreSQL** destination instance (`14+` recommended)

## Option 1: Clone the repository (source-first)

```bash
git clone <repository-url> cockroach-migrate-tool
cd cockroach-migrate-tool
```

The workspace contains four crates:

| Crate | Binary / Library | Purpose |
|-------|------------------|---------|
| `crates/runner` | `runner` binary | Long-running webhook receiver and reconcile engine |
| `crates/setup-sql` | `setup-sql` binary | One-time SQL emission CLI |
| `crates/ingest-contract` | Library | Shared URL routing contract between setup-sql and runner |
| `crates/operator-log` | Library | Structured JSON/text logging primitives used by both binaries |

### Build everything

```bash
cargo build --release
```

The release binaries will be placed in `target/release/runner` and `target/release/setup-sql`.

### Run tests

```bash
cargo test --workspace
```

### Run a single binary directly

```bash
# Validate a runner config
./target/release/runner validate-config --config ./my-runner.yml --deep

# Start the runner
./target/release/runner run --config ./my-runner.yml

# Emit CockroachDB changefeed SQL
./target/release/setup-sql emit-cockroach-sql --config ./cockroach-setup.yml

# Emit PostgreSQL grants
./target/release/setup-sql emit-postgres-grants --config ./postgres-grants.yml
```

## Option 2: Build container images

Dockerfiles are not shipped in the repository, but the test suite and CI artifacts assume standard multi-stage builds. A minimal `Dockerfile` for the runner looks like this:

```dockerfile
# Build stage
FROM rust:1.93 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p runner

# Runtime stage
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/runner /usr/local/bin/runner
ENTRYPOINT ["runner"]
```

And for `setup-sql`:

```dockerfile
FROM rust:1.93 AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p setup-sql

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/setup-sql /usr/local/bin/setup-sql
ENTRYPOINT ["setup-sql"]
```

Build and tag them:

```bash
docker build -f Dockerfile.runner -t migration-runner:latest .
docker build -f Dockerfile.setup-sql -t migration-setup-sql:latest .
```

## Option 3: Use the provided Compose files

The repository ships with reference Compose files under `artifacts/compose/` that you can adapt for your own deployment.

| File | Purpose |
|------|---------|
| `runner.compose.yml` | Deploys the `runner` service with mounted configs and TLS certificates |
| `setup-sql.compose.yml` | Runs `setup-sql` as a one-off job to emit SQL |
| `verify.compose.yml` | Deploys a verification service (if you use the separate verify binary) |

Example adaptation:

```bash
export RUNNER_IMAGE=migration-runner:latest
export RUNNER_HTTPS_PORT=8443
docker compose -f artifacts/compose/runner.compose.yml up -d
```

## Option 4: Install via `cargo install` from a local path

If you prefer to install the binaries into `~/.cargo/bin`:

```bash
cargo install --path crates/runner
cargo install --path crates/setup-sql
```

After installation the commands `runner` and `setup-sql` will be available on your `PATH` (assuming `~/.cargo/bin` is configured).

## Verifying the installation

Both binaries support a `--help` flag:

```bash
runner --help
setup-sql --help
```

You can also do a shallow config validation without starting any network listeners:

```bash
runner validate-config --config ./runner.yml
```

For a deep validation that also checks connectivity to all destination PostgreSQL databases and confirms the destination tables exist:

```bash
runner validate-config --config ./runner.yml --deep
```

## TLS certificate setup (quick checklist)

Because CockroachDB changefeed webhooks require HTTPS, you need TLS material for the runner:

1. **Server certificate and key** — standard PEM-encoded files referenced by `webhook.tls.cert_path` and `webhook.tls.key_path`.
2. **CA certificate** — the certificate that signed the runner server certificate. This is referenced by `setup-sql` (`webhook.ca_cert_path`) so CockroachDB can verify the runner.
3. **Optional: Client CA** — if you want mutual TLS, set `webhook.tls.client_ca_path`. CockroachDB does not natively send client certificates for webhook changefeeds, so this is only needed if you terminate TLS behind a proxy that performs mTLS.

For PostgreSQL destination TLS:

1. **CA certificate** — required when `mode` is `verify-ca` or `verify-full`.
2. **Optional: Client certificate + key** — if your PostgreSQL instance requires certificate authentication.

## Uninstallation

If you installed with `cargo install`:

```bash
cargo uninstall runner
cargo uninstall setup-sql
```

If you used Docker:

```bash
docker rm -f migration-runner
docker rmi migration-runner:latest migration-setup-sql:latest
```

To remove the helper schema from a destination database after a migration is complete:

```sql
DROP SCHEMA _cockroach_migration_tool CASCADE;
```
