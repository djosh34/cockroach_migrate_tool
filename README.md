# Cockroach Migrate Tool

This repository contains the first Rust workspace for the CockroachDB-to-PostgreSQL migration runner.

## Workspace Layout

- `crates/runner`: destination-side runtime for validated config loading, PostgreSQL access wiring, webhook runtime wiring, and reconcile runtime wiring.
- `crates/source-bootstrap`: source-side CLI for rendering the CockroachDB bootstrap script from typed YAML config.
- `Dockerfile`: single-binary destination image for the `runner` runtime.

## Source Bootstrap Quick Start

The source-side bootstrap stays explicit. Render the CockroachDB setup script, review it, then execute it yourself; the tool does not hide source-side commands behind the CLI.

Example source bootstrap config:

```yaml
# config/source-bootstrap.yml
cockroach:
  url: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require
webhook:
  url: https://runner.example.internal:8443/events
  resolved: 5s
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
```

Render the runnable bootstrap script:

```bash
cargo run -p source-bootstrap -- \
  render-bootstrap-script \
  --config config/source-bootstrap.yml > cockroach-bootstrap.sh
```

The rendered script:

- enables `kv.rangefeed.enabled`
- captures `cluster_logical_timestamp()`
- creates one webhook changefeed per configured source database
- prints mapping id, source database, selected tables, starting cursor, and job id after each changefeed is created

## Docker Quick Start

The destination runtime is one container that starts the `runner` binary directly. There is no wrapper shell script in the user path.

1. Create a config directory with one runner config file and the TLS material it references.

```yaml
# config/runner.yml
webhook:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
reconcile:
  interval_secs: 30
verify:
  molt:
    command: molt
    report_dir: /work/molt
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
    destination:
      connection:
        host: pg-a.example.internal
        port: 5432
        database: app_a
        user: migration_user_a
        password: runner-secret-a
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
    destination:
      connection:
        host: pg-b.example.internal
        port: 5432
        database: app_b
        user: migration_user_b
        password: runner-secret-b
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

4. Validate the exported CockroachDB and PostgreSQL schemas semantically before starting the runtime. The compare command uses the selected mapping’s table list as the filter, ignores unrelated tables, and rejects structural mismatches without relying on a raw text diff.

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  -v "$(pwd)/schema:/schema:ro" \
  cockroach-migrate-runner \
  compare-schema \
  --config /config/runner.yml \
  --mapping app-a \
  --cockroach-schema /schema/crdb_schema.txt \
  --postgres-schema /schema/pg_schema.sql
```

5. Render the PostgreSQL grant artifacts and review the generated `README.md` plus per-mapping `grants.sql` files. These grants stay manual and explicit; no superuser role is assumed.

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  -v "$(pwd)/postgres-setup:/work/postgres-setup" \
  cockroach-migrate-runner \
  render-postgres-setup --config /config/runner.yml --output-dir /work/postgres-setup
```

6. Start the destination runtime directly through the image entrypoint. On startup, `runner run --config <path>` connects to each destination database, creates `_cockroach_migration_tool`, creates the tracking tables, prepares helper shadow tables, and adds the automatic minimal PK helper indexes when they are needed.

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  cockroach-migrate-runner \
  run --config /config/runner.yml
```

The mounted `/config` directory is the only Docker-specific convention. The same `runner validate-config --config <path>`, `runner compare-schema --config <path> --mapping <id> --cockroach-schema <path> --postgres-schema <path>`, `runner render-postgres-setup --config <path> --output-dir <dir>`, and `runner run --config <path>` interface remains the public contract on the host and in the container.

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
