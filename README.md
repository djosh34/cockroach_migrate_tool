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
  base_url: https://runner.example.internal:8443
  ca_cert_path: ca.crt
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

Run the rendered script yourself after review:

```bash
bash cockroach-bootstrap.sh
```

The rendered script:

- enables `kv.rangefeed.enabled`
- captures `cluster_logical_timestamp()`
- creates one webhook changefeed per configured source database
- renders each mapping to its own HTTPS ingest path at `/ingest/<mapping_id>`
- prints mapping id, source database, selected tables, starting cursor, and job id after each changefeed is created

## Docker Quick Start

The destination runtime is one container that starts the `runner` binary directly. There is no wrapper shell script in the user path.
You should not need to inspect `crates/`, `tests/`, or `investigations/` to complete this quick start.

1. Create a config directory, generate TLS material for local testing, and write one runner config file.

```bash
mkdir -p config/certs
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout config/certs/server.key \
  -out config/certs/server.crt \
  -days 365 \
  -subj "/CN=runner.example.internal"
```

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

4. Export the source and destination schema artifacts into a local `schema/` directory before running any schema-aware runner command. For each mapping, use that mapping's CockroachDB source database and PostgreSQL destination database.

```bash
mkdir -p schema
cockroach sql \
  --url "postgresql://root@crdb.example.internal:26257/demo_a?sslmode=require" \
  --execute "SHOW CREATE ALL TABLES;" > schema/crdb_schema.txt
pg_dump \
  --schema-only \
  --no-owner \
  --no-privileges \
  --dbname "postgresql://postgres@pg-a.example.internal:5432/app_a?sslmode=require" \
 > schema/pg_schema.sql
```

Keep using the same `/config/runner.yml`, `app-a`, `/schema/crdb_schema.txt`, and `/schema/pg_schema.sql` values in the remaining quick-start commands unless you intentionally switch to a different mapping.

5. Validate the exported CockroachDB and PostgreSQL schemas semantically before starting the runtime. The compare command uses the selected mapping’s table list as the filter, ignores unrelated tables, and rejects structural mismatches without relying on a raw text diff.

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

6. Render the PostgreSQL grant artifacts, review the generated `README.md`, and run each `grants.sql` before starting the runtime. These grants stay manual and explicit; no superuser role is assumed.

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  -v "$(pwd)/postgres-setup:/work/postgres-setup" \
  cockroach-migrate-runner \
  render-postgres-setup --config /config/runner.yml --output-dir /work/postgres-setup
```

7. Render the helper shadow-table plan and reconcile order for one validated mapping when you want explicit review artifacts before runtime bootstrap. The render command reuses semantic schema validation and writes `helper_tables.sql`, `reconcile_order.txt`, and a per-mapping `README.md`.

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  -v "$(pwd)/schema:/schema:ro" \
  -v "$(pwd)/helper-plan:/work/helper-plan" \
  cockroach-migrate-runner \
  render-helper-plan \
  --config /config/runner.yml \
  --mapping app-a \
  --cockroach-schema /schema/crdb_schema.txt \
  --postgres-schema /schema/pg_schema.sql \
  --output-dir /work/helper-plan
```

8. Start the destination runtime directly through the image entrypoint. On startup, `runner run --config <path>` connects to each destination database, creates `_cockroach_migration_tool`, creates the tracking tables, prepares helper shadow tables from the same helper-plan rules as `render-helper-plan`, adds the automatic minimal PK helper indexes when they are needed, and then keeps serving HTTPS from the same process.

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  cockroach-migrate-runner \
  run --config /config/runner.yml
```

After startup, the runtime serves:

- `GET /healthz`
- `POST /ingest/<mapping_id>`

The mounted `/config` directory is the only Docker-specific convention. The same `runner validate-config --config <path>`, `runner compare-schema --config <path> --mapping <id> --cockroach-schema <path> --postgres-schema <path>`, `runner render-postgres-setup --config <path> --output-dir <dir>`, `runner render-helper-plan --config <path> --mapping <id> --cockroach-schema <path> --postgres-schema <path> --output-dir <dir>`, and `runner run --config <path>` interface remains the public contract on the host and in the container.

## Write-Freeze Cutover Runbook

Use one mapping-scoped runbook for cutover. Before handover starts, run `runner verify --config <path> --mapping <id> --source-url <cockroach-url>` repeatedly while PostgreSQL is shadowing CockroachDB so parity is already a normal check, not a last-minute surprise.

1. Block writes at the API boundary for the mapping you are handing over.
2. Run `runner cutover-readiness --config <path> --mapping <id> --source-url <cockroach-url>` until it reports `ready=true`.
3. Run one final `runner verify --config <path> --mapping <id> --source-url <cockroach-url>` after readiness reports drained.
4. Switch application traffic to PostgreSQL only after those checks finish cleanly.

Do not switch traffic until writes are frozen, `runner cutover-readiness` has drained to zero with `ready=true`, and the final `runner verify` reports equality.

## CI Publish Safety

Random pull requests, forks, `pull_request_target`, manual dispatch, reusable workflow calls, scheduled runs, and tag pushes do not trigger the protected image-publish workflow.

The `publish` job still carries an explicit `if:` gate that requires a `push` event on `refs/heads/master`, so widening workflow triggers later does not silently open the release path.

Only the `publish` job gets `packages: write`, checkout disables credential persistence, and the pushed image is tagged only with `${{ github.sha }}` from the validated commit.

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
