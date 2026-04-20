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

## Setup SQL Docker Compose

Copy this file next to a `config/` directory containing `cockroach-setup.yml`, `postgres-grants.yml`, and `ca.crt`. This compose contract still starts from published images only; no repository checkout or local image build is required.

```bash
export GITHUB_OWNER=<github-owner>
export IMAGE_TAG=<published-commit-sha>
export SETUP_SQL_IMAGE="ghcr.io/${GITHUB_OWNER}/cockroach-migrate-setup-sql:${IMAGE_TAG}"
```

Save this as `setup-sql.compose.yml`:

```yaml
services:
  setup-sql:
    image: "${SETUP_SQL_IMAGE}"
    configs:
      - source: cockroach-setup-config
        target: /config/cockroach-setup.yml
      - source: postgres-grants-config
        target: /config/postgres-grants.yml
      - source: source-ca-cert
        target: /config/ca.crt
    command:
      - emit-cockroach-sql
      - --config
      - /config/cockroach-setup.yml

configs:
  cockroach-setup-config:
    file: ./config/cockroach-setup.yml
  postgres-grants-config:
    file: ./config/postgres-grants.yml
  source-ca-cert:
    file: ./config/ca.crt
```

Render the SQL artifacts directly through Docker Compose:

```bash
docker compose -f setup-sql.compose.yml run --rm setup-sql > cockroach-bootstrap.sql
docker compose -f setup-sql.compose.yml run --rm setup-sql emit-postgres-grants --config /config/postgres-grants.yml > postgres-grants.sql
```

## Runner Docker Compose

Copy this file next to a `config/` directory containing `runner.yml`, `certs/server.crt`, and `certs/server.key`. This compose contract still starts from published images only; no repository checkout or local image build is required.

```bash
export GITHUB_OWNER=<github-owner>
export IMAGE_TAG=<published-commit-sha>
export RUNNER_IMAGE="ghcr.io/${GITHUB_OWNER}/cockroach-migrate-runner:${IMAGE_TAG}"
```

Save this as `runner.compose.yml`:

```yaml
services:
  runner:
    image: "${RUNNER_IMAGE}"
    ports:
      - "${RUNNER_HTTPS_PORT:-8443}:8443"
    configs:
      - source: runner-config
        target: /config/runner.yml
      - source: runner-server-cert
        target: /config/certs/server.crt
      - source: runner-server-key
        target: /config/certs/server.key
    command:
      - run
      - --config
      - /config/runner.yml

configs:
  runner-config:
    file: ./config/runner.yml
  runner-server-cert:
    file: ./config/certs/server.crt
  runner-server-key:
    file: ./config/certs/server.key
```

Validate the mounted config and then start the runtime:

```bash
docker compose -f runner.compose.yml run --rm runner validate-config --config /config/runner.yml
docker compose -f runner.compose.yml up runner
```

## Verify Docker Compose

Copy this file next to a `config/` directory containing `verify-service.yml` and the certificate paths it references under `config/certs/`. This compose contract starts the published verify-service API directly; no repository checkout or local image build is required.

```bash
export GITHUB_OWNER=<github-owner>
export IMAGE_TAG=<published-commit-sha>
export VERIFY_IMAGE="ghcr.io/${GITHUB_OWNER}/cockroach-migrate-verify:${IMAGE_TAG}"
```

Save this as `verify.compose.yml`:

```yaml
services:
  verify:
    image: "${VERIFY_IMAGE}"
    ports:
      - "${VERIFY_HTTPS_PORT:-9443}:8080"
    configs:
      - source: verify-service-config
        target: /config/verify-service.yml
      - source: verify-source-ca
        target: /config/certs/source-ca.crt
      - source: verify-source-client-cert
        target: /config/certs/source-client.crt
      - source: verify-source-client-key
        target: /config/certs/source-client.key
      - source: verify-destination-ca
        target: /config/certs/destination-ca.crt
      - source: verify-server-cert
        target: /config/certs/server.crt
      - source: verify-server-key
        target: /config/certs/server.key
    command:
      - --config
      - /config/verify-service.yml

configs:
  verify-service-config:
    file: ./config/verify-service.yml
  verify-source-ca:
    file: ./config/certs/source-ca.crt
  verify-source-client-cert:
    file: ./config/certs/source-client.crt
  verify-source-client-key:
    file: ./config/certs/source-client.key
  verify-destination-ca:
    file: ./config/certs/destination-ca.crt
  verify-server-cert:
    file: ./config/certs/server.crt
  verify-server-key:
    file: ./config/certs/server.key
```

Start the dedicated verify-service API:

```bash
docker compose -f verify.compose.yml up verify
```

## CI Publish Safety

Random pull requests, forks, `pull_request_target`, manual dispatch, reusable workflow calls, scheduled runs, tag pushes, issue-triggered events, and release events do not trigger the protected image-publish workflow.

The `publish-image`, `quay-security-gate`, and `publish-manifest` jobs still carry explicit `if:` gates that require a `push` event on `refs/heads/main`, so widening workflow triggers later does not silently open the protected release path.

Only the `publish-manifest` job gets `packages: write`, checkout disables credential persistence where source is fetched, Quay login uses `--password-stdin`, the Quay scan step uses a temporary netrc file instead of command-line passwords, and every canonical published image still resolves to `${{ github.sha }}` from the validated commit.

Image publication is blocked on explicit `validate-fast` and `validate-long` jobs, so both the default repository validation boundary and the ultra-long lane must pass before any publish step can start.

Both validation jobs restore and save Cargo registry and target caches before publish, each image is first pushed through native `linux/amd64` and `linux/arm64` Quay lanes, the `quay-security-gate` job polls Quay manifest security until every published platform ref is scanned with zero findings, and only then does the manifest job assemble canonical Quay `${{ github.sha }}` tags and fan them out into GHCR while emitting a published-image manifest for downstream consumers.
