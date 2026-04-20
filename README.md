# Cockroach Migrate Tool

Run the published `setup-sql`, `runner`, and `verify` images with inline configs only. No repository checkout, local Rust install, or local image build is required.

## Setup SQL Quick Start

Pull the published `setup-sql` image, render the SQL you need, review it, and apply it yourself. The one-time setup flow stays separate from the long-running runtime.

```bash
export GITHUB_OWNER=<github-owner>
export IMAGE_TAG=<published-commit-sha>
export SETUP_SQL_IMAGE="ghcr.io/${GITHUB_OWNER}/cockroach-migrate-setup-sql:${IMAGE_TAG}"
docker pull "${SETUP_SQL_IMAGE}"
```

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
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${SETUP_SQL_IMAGE}" \
  emit-cockroach-sql \
  --log-format json \
  --config /config/cockroach-setup.yml > cockroach-bootstrap.sql
```

Required args:

- `emit-cockroach-sql`
- `--config /config/cockroach-setup.yml`

Optional args:

- `--log-format json` for structured stderr logs while stdout stays reserved for SQL

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
  --log-format json \
  --config /config/postgres-grants.yml > postgres-grants.sql
```

Required args:

- `emit-postgres-grants`
- `--config /config/postgres-grants.yml`

Optional args:

- `--log-format json` for structured stderr logs while stdout stays reserved for SQL

Apply the emitted PostgreSQL grant SQL before starting the runtime:

```bash
psql \
  "postgresql://postgres-admin@pg-a.example.internal:5432/app_a?sslmode=require" \
  -f postgres-grants.sql
```

If you prefer Docker Compose, save the same image contract inline and reuse the same config files.

Save this as `setup-sql.compose.yml`:

```yaml
services:
  setup-sql:
    image: "${SETUP_SQL_IMAGE}"
    network_mode: none
    configs:
      - source: cockroach-setup-config
        target: /config/cockroach-setup.yml
      - source: postgres-grants-config
        target: /config/postgres-grants.yml
      - source: source-ca-cert
        target: /config/ca.crt
    command:
      - emit-cockroach-sql
      - --log-format
      - json
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

Render the SQL artifacts with Compose:

```bash
docker compose -f setup-sql.compose.yml run --rm setup-sql > cockroach-bootstrap.sql
docker compose -f setup-sql.compose.yml run --rm setup-sql emit-postgres-grants --log-format json --config /config/postgres-grants.yml > postgres-grants.sql
```

## Runner Quick Start

Pull the published runner image and create `config/certs/server.crt`, `config/certs/server.key`, `config/certs/destination-ca.crt`, `config/certs/destination-client.crt`, and `config/certs/destination-client.key`.

```bash
export GITHUB_OWNER=<github-owner>
export IMAGE_TAG=<published-commit-sha>
export RUNNER_IMAGE="ghcr.io/${GITHUB_OWNER}/cockroach-migrate-runner:${IMAGE_TAG}"
docker pull "${RUNNER_IMAGE}"
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
      tls:
        mode: verify-ca
        ca_cert_path: /config/certs/destination-ca.crt
        client_cert_path: /config/certs/destination-client.crt
        client_key_path: /config/certs/destination-client.key
```

Validate the mounted config directly through the image entrypoint:

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --log-format json --config /config/runner.yml
```

Required args:

- `validate-config --config /config/runner.yml`
- `run --config /config/runner.yml`

Optional args:

- `--log-format json` for structured stderr logs

Before starting the runtime, apply the PostgreSQL grant SQL from the setup section. Then start the runtime directly through the image entrypoint.

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  run --log-format json --config /config/runner.yml
```

The runtime serves:

- `GET /healthz`
- `POST /ingest/<mapping_id>`

If you prefer Compose, use the same image contract with Docker Compose.

Save this as `runner.compose.yml`:

```yaml
services:
  runner:
    image: "${RUNNER_IMAGE}"
    network_mode: bridge
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
      - --log-format
      - json
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

Validate the mounted config and then start the runtime with Compose:

```bash
docker compose -f runner.compose.yml run --rm runner validate-config --log-format json --config /config/runner.yml
docker compose -f runner.compose.yml up runner
```

## Verify Quick Start

Pull the published verify image and write the verify-service config inline. Create `config/certs/source-ca.crt`, `config/certs/source-client.crt`, `config/certs/source-client.key`, `config/certs/destination-ca.crt`, `config/certs/client-ca.crt`, `config/certs/server.crt`, and `config/certs/server.key`.

```bash
export GITHUB_OWNER=<github-owner>
export IMAGE_TAG=<published-commit-sha>
export VERIFY_IMAGE="ghcr.io/${GITHUB_OWNER}/cockroach-migrate-verify:${IMAGE_TAG}"
docker pull "${VERIFY_IMAGE}"
mkdir -p config/certs
```

Example verify service config:

```yaml
# config/verify-service.yml
listener:
  bind_addr: 0.0.0.0:8080
  transport:
    mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_auth:
      mode: mtls
      client_ca_path: /config/certs/client-ca.crt
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      mode: verify-full
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    url: postgresql://verify_target@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      mode: verify-ca
      ca_cert_path: /config/certs/destination-ca.crt
```

Start the verify API directly:

```bash
docker run --rm \
  -p 9443:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  --log-format json \
  --config /config/verify-service.yml
```

Required args:

- `--config /config/verify-service.yml`

Optional args:

- `--log-format json` for structured stderr logs

If you prefer Compose, use the same image contract with Docker Compose.

Save this as `verify.compose.yml`:

```yaml
services:
  verify:
    image: "${VERIFY_IMAGE}"
    network_mode: bridge
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
      - source: verify-client-ca
        target: /config/certs/client-ca.crt
      - source: verify-server-cert
        target: /config/certs/server.crt
      - source: verify-server-key
        target: /config/certs/server.key
    command:
      - --log-format
      - json
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
  verify-client-ca:
    file: ./config/certs/client-ca.crt
  verify-server-cert:
    file: ./config/certs/server.crt
  verify-server-key:
    file: ./config/certs/server.key
```

Start the dedicated verify API with Compose:

```bash
docker compose -f verify.compose.yml up verify
```
