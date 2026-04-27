# Installation

This project is not published as packages from the code shown here. Installation means obtaining the source and building the binaries or Docker images.

## Get The Source

Clone over SSH:

```sh
git clone git@github.com:<owner>/<repo>.git cockroach_migrate_tool
cd cockroach_migrate_tool
```

Clone over HTTPS:

```sh
git clone https://github.com/<owner>/<repo>.git cockroach_migrate_tool
cd cockroach_migrate_tool
```

Use an existing checkout:

```sh
cd /home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool
```

Use a source archive:

```sh
curl -L -o cockroach_migrate_tool.tar.gz https://github.com/<owner>/<repo>/archive/<ref>.tar.gz
tar -xzf cockroach_migrate_tool.tar.gz
cd <repo>-<ref>
```

Replace `<owner>`, `<repo>`, and `<ref>` with the actual repository owner, repository name, and branch, tag, or commit.

## Toolchains

The Rust workspace requires:

- Rust `1.93`.
- Cargo.

The vendored Molt verifier requires:

- Go `1.26` when using the same toolchain as the Dockerfile.
- Or `GOTOOLCHAIN=auto` for local Go tests, matching the `Makefile`.

Docker builds install their own toolchains inside builder stages.

## Build Rust Binaries Locally

Build every Rust crate:

```sh
cargo build --workspace
```

Build release binaries:

```sh
cargo build --locked --release -p runner --bin runner
cargo build --locked --release -p setup-sql --bin setup-sql
```

The binaries are written to:

```text
target/release/runner
target/release/setup-sql
```

Run them directly:

```sh
target/release/runner validate-config --config config/runner.yml
target/release/setup-sql emit-cockroach-sql --config config/cockroach-setup.yml
```

## Build Verify Service Locally

The verify service lives in the vendored Molt subtree:

```sh
cd cockroachdb_molt/molt
go build -trimpath -o molt .
./molt verify-service validate-config --config ../../config/verify-service.yml
```

From the repository root, the Go package tested by the project is:

```sh
cd cockroachdb_molt/molt
GOTOOLCHAIN=auto go test ./cmd/verifyservice -count=1
```

## Build Docker Images

Build the runner image:

```sh
docker build -t cockroach-migrate-runner:local -f Dockerfile .
```

Build the setup SQL image:

```sh
docker build -t cockroach-migrate-setup-sql:local -f crates/setup-sql/Dockerfile .
```

Build the verify service image:

```sh
docker build -t cockroach-migrate-verify:local -f cockroachdb_molt/molt/Dockerfile cockroachdb_molt/molt
```

Image entrypoints:

| Image | Entrypoint | Example command |
| --- | --- | --- |
| `cockroach-migrate-runner:local` | `/usr/local/bin/runner` | `validate-config --config /config/runner.yml` |
| `cockroach-migrate-setup-sql:local` | `/usr/local/bin/setup-sql` | `emit-cockroach-sql --config /config/cockroach-setup.yml` |
| `cockroach-migrate-verify:local` | `/usr/local/bin/molt verify-service` | `validate-config --config /config/verify-service.yml` |

## Compose Artifacts

The repository includes three Compose files:

| File | Service | Required image env var | Main mounted config |
| --- | --- | --- | --- |
| `artifacts/compose/runner.compose.yml` | `runner` | `RUNNER_IMAGE` | `./config/runner.yml` |
| `artifacts/compose/setup-sql.compose.yml` | `setup-sql` | `SETUP_SQL_IMAGE` | `./config/cockroach-setup.yml` |
| `artifacts/compose/verify.compose.yml` | `verify` | `VERIFY_IMAGE` | `./config/verify-service.yml` |

Examples:

```sh
RUNNER_IMAGE=cockroach-migrate-runner:local \
docker compose -f artifacts/compose/runner.compose.yml up
```

```sh
SETUP_SQL_IMAGE=cockroach-migrate-setup-sql:local \
docker compose -f artifacts/compose/setup-sql.compose.yml run --rm setup-sql
```

```sh
VERIFY_IMAGE=cockroach-migrate-verify:local \
docker compose -f artifacts/compose/verify.compose.yml up
```

## Validation And Tests

Run the same project checks exposed by the `Makefile`:

```sh
make lint
make test
```

`make lint` runs:

```sh
cargo clippy --workspace --all-targets -- -D warnings
```

`make test` runs:

```sh
cargo test --workspace
cd cockroachdb_molt/molt && GOTOOLCHAIN=auto go test ./cmd/verifyservice -count=1
```

There is also a long-test target:

```sh
make test-long
```

It runs ignored Rust tests with one test thread:

```sh
cargo test --workspace -- --ignored --test-threads=1
```

## Runtime Validation Commands

Validate runner YAML only:

```sh
runner validate-config --config config/runner.yml
```

Validate runner YAML and destination catalog access:

```sh
runner validate-config --config config/runner.yml --deep
```

Validate verify-service config:

```sh
molt verify-service validate-config --config config/verify-service.yml
```

Emit setup SQL without contacting databases:

```sh
setup-sql emit-cockroach-sql --config config/cockroach-setup.yml
setup-sql emit-postgres-grants --config config/postgres-grants.yml
```

All three CLIs support operator JSON logs via:

```sh
--log-format json
```

