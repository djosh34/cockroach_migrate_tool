# Runner: Getting Started

Pull the runner image, write its configuration, validate it, and start the runtime.

## Before you begin

Complete these steps first:

1. [CockroachDB source setup](../source-setup/cockroachdb-setup.md) — rangefeeds enabled, cursors captured, changefeeds created.
2. [PostgreSQL destination grants](../destination-setup/postgresql-grants.md) — the runtime role has `CONNECT`, `CREATE`, `USAGE`, and `SELECT`/`INSERT`/`UPDATE`/`DELETE` on every mapped table.

> The runner refuses to start correctly if changefeeds are not already pointing at it, or if the destination role cannot connect and write.

## 1. Pull the image

```bash
export GITHUB_OWNER=<owner>
export IMAGE_TAG=<published-commit-sha>
export RUNNER_IMAGE="ghcr.io/${GITHUB_OWNER}/runner-image:${IMAGE_TAG}"

docker pull "${RUNNER_IMAGE}"
```

## 2. Write configuration

Create `config/runner.yml`. The minimal configuration for an HTTPS webhook listener:

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
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

For a complete field reference, see [Runner configuration](./configuration.md).

### HTTP mode (local development only)

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      url: postgresql://migration_user_a:runner-secret-a@pg-a:5432/app_a
```

## 3. Validate configuration

Always validate before running. Offline validation checks config structure and field values:

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml
```

To additionally verify that each destination database is reachable and every mapped table exists, add `--deep`:

```bash
docker run --rm \
  --network host \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml --deep
```

> `--deep` requires network access to the destination databases. The plain `validate-config` command is fully offline.

## 4. Run

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  run --config /config/runner.yml
```

For structured JSON logs, add `--log-format json` before the subcommand:

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  --log-format json \
  run --config /config/runner.yml
```

## 5. Confirm it is running

```bash
curl -k https://localhost:8443/healthz
```

For HTTP mode, use `http://localhost:8080/healthz`.

See [Runner endpoints](./endpoints.md) for all available endpoints.

## Docker Compose

Save as `runner.compose.yml`:

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

```bash
docker compose -f runner.compose.yml run --rm runner validate-config --config /config/runner.yml
docker compose -f runner.compose.yml up runner
```

## See also

- [Runner configuration](./configuration.md) — full YAML reference
- [Runner endpoints](./endpoints.md) — `/healthz`, `/metrics`, `/ingest/{mapping_id}`
- [CockroachDB source setup](../source-setup/cockroachdb-setup.md) — changefeeds that target `/ingest/{mapping_id}`
- [TLS reference](../tls-reference.md) — webhook listener and destination TLS settings