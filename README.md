# Cockroach Migrate Tool

This repository contains the first Rust workspace for the CockroachDB-to-PostgreSQL migration runner.
For contributor workflow, see `CONTRIBUTING.md`.

## Licensing

The Rust workspace and repository-root material are proprietary:
All Rights Reserved - Joshua Azimullah.

The vendored verify-derived component under `cockroachdb_molt/molt` remains
under the Apache License, Version 2.0. The retained Apache-2.0 text lives at
`cockroachdb_molt/molt/LICENSE`, and the repository-level split is summarized
in `THIRD_PARTY_NOTICES`.

## Setup SQL Quick Start

The supported novice-user path starts from pulling published images only. This flow does not require a repository checkout, a local Rust install, or any image build from source.

The one-time setup flow stays explicit. Pull the published `setup-sql` image, emit the required SQL, review it, then apply it yourself with a CockroachDB or PostgreSQL client. The runtime image never absorbs one-time setup powers.

Example Cockroach setup config:

```yaml
# config/cockroach-setup.yml
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

Render the Cockroach bootstrap SQL:

```bash
export GITHUB_OWNER=<github-owner>
export IMAGE_TAG=<published-commit-sha>
export SETUP_SQL_IMAGE="ghcr.io/${GITHUB_OWNER}/cockroach-migrate-setup-sql:${IMAGE_TAG}"
docker pull "${SETUP_SQL_IMAGE}"
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${SETUP_SQL_IMAGE}" \
  emit-cockroach-sql \
  --config /config/cockroach-setup.yml > cockroach-bootstrap.sql
```

Apply the rendered SQL yourself after review:

```bash
cockroach sql \
  --url 'postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require' \
  --file cockroach-bootstrap.sql
```

The rendered SQL:

- enables `kv.rangefeed.enabled`
- records `cluster_logical_timestamp()` as an explicit source-side statement and feeds that value back into each changefeed `cursor`
- creates one webhook changefeed per configured source database
- renders each mapping to its own HTTPS ingest path at `/ingest/<mapping_id>`
- keeps the operator-facing artifact to SQL statements plus SQL comments only

Example PostgreSQL grants config:

```yaml
# config/postgres-grants.yml
mappings:
  - id: app-a
    destination:
      database: app_a
      runtime_role: migration_user_a
      tables:
        - public.customers
        - public.orders
```

Render the PostgreSQL grants SQL:

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${SETUP_SQL_IMAGE}" \
  emit-postgres-grants \
  --config /config/postgres-grants.yml > postgres-grants.sql
```

Apply the emitted PostgreSQL grant SQL before starting the runtime:

```bash
psql \
  "postgresql://postgres-admin@pg-a.example.internal:5432/app_a?sslmode=require" \
  -f postgres-grants.sql
```

## Docker Quick Start

The destination runtime is one published container that starts the `runner` binary directly. There is no wrapper shell script in the user path.
You should not need to inspect `crates/`, `tests/`, or `investigations/` to complete this quick start.

1. Choose the published image coordinates and pull the validated runner image.

```bash
export GITHUB_OWNER=<github-owner>
export IMAGE_TAG=<published-commit-sha>
export RUNNER_IMAGE="ghcr.io/${GITHUB_OWNER}/cockroach-migrate-runner:${IMAGE_TAG}"
docker pull "${RUNNER_IMAGE}"
```

2. Create a config directory, generate TLS material for local testing, and write one runner config file.

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
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
    destination:
      host: pg-a.example.internal
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
```

3. Validate the mounted config directly through the image entrypoint:

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml
```

4. Before starting the runtime, use the `setup-sql` quick start above to emit the PostgreSQL grants, review them, and apply the emitted PostgreSQL grant SQL before starting the runtime. These grants stay manual and explicit; no superuser role is assumed.

5. Start the destination runtime directly through the image entrypoint. On startup, `runner run --config <path>` connects to each destination database, creates `_cockroach_migration_tool`, creates the tracking tables, derives helper shadow tables from destination catalog state, adds the automatic minimal PK helper indexes when they are needed, and then keeps serving HTTPS from the same process.

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  run --config /config/runner.yml
```

After startup, the runtime serves:

- `GET /healthz`
- `POST /ingest/<mapping_id>`

The mounted `/config` directory is the only Docker-specific convention. The same `runner validate-config --config <path>` and `runner run --config <path>` interface remains the public contract on the host and in the container.

## CI Publish Safety

Random pull requests, forks, `pull_request_target`, manual dispatch, reusable workflow calls, scheduled runs, and tag pushes do not trigger the protected image-publish workflow.

The `publish` job still carries an explicit `if:` gate that requires a `push` event on `refs/heads/master`, so widening workflow triggers later does not silently open the release path.

Only the `publish` job gets `packages: write`, checkout disables credential persistence, and the pushed images are tagged only with `${{ github.sha }}` from the validated commit.

Before any push, the workflow builds each release-image archive, scans those exact archives with Trivy, fails on `HIGH` or `CRITICAL` findings, and always uploads the scan report artifacts for review.
