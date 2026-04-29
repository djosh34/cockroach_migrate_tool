# Runner

The runner image receives CockroachDB changefeed webhook batches and writes the incoming row mutations into PostgreSQL destination tables. It also runs a periodic reconciliation loop over each destination table.

For a deeper explanation of webhook ingestion, the reconcile loop, and the `_cockroach_migration_tool` helper schema, see [Architecture](architecture.md).

## Quick start

```bash
export GITHUB_OWNER="<your-github-org-or-user>"
export RUNNER_IMAGE="ghcr.io/${GITHUB_OWNER}/runner-image:<git-sha>"
docker pull "${RUNNER_IMAGE}"

docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  run --config /config/runner.yml
```

Always validate config before running. See [full walkthrough](#end-to-end-walkthrough) below.

## CLI

```
runner [--log-format text|json] validate-config --config <PATH> [--deep]
runner [--log-format text|json] run --config <PATH>
```

| Subcommand | Purpose | Flags |
|------------|---------|-------|
| `validate-config` | Check config structure and field values (offline) | `--config <PATH>` (required), `--deep` (optional) |
| `run` | Start the webhook listener and reconciliation loop | `--config <PATH>` (required) |

- `--log-format json` outputs structured JSON on stderr. Default is `text`.
- `--deep` added to `validate-config` additionally verifies each destination database is reachable and every mapped table exists. Requires network access.

## Configuration reference

The runner reads a single YAML file passed via `--config <PATH>`.

### Top-level structure

```yaml
webhook: ...
reconcile: ...
mappings:
  - id: ...
    source: ...
    destination: ...
```

All three keys are required.

### `webhook`

Controls the listener that receives CockroachDB changefeed batches.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `bind_addr` | string | yes | Host and port, e.g. `0.0.0.0:8443` |
| `mode` | string | no | `http` or `https`. Defaults to `https`. |
| `tls` | object | yes for `mode: https` | Server TLS configuration. Must not appear when `mode: http`. |

#### `webhook.tls`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cert_path` | path | yes | Server certificate PEM file |
| `key_path` | path | yes | Server private key PEM file |
| `client_ca_path` | path | no | CA certificate for mTLS client verification |

#### Examples

HTTPS (production):

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

HTTPS with mTLS:

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

HTTP (development only):

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
```

### `reconcile`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `interval_secs` | integer | yes | Seconds between reconciliation passes. Must be > 0. |

```yaml
reconcile:
  interval_secs: 30
```

Reconciliation copies rows from `_cockroach_migration_tool` shadow tables into real destination tables using upsert and delete passes. The interval controls how often this happens. See [Architecture — Reconciliation loop](architecture.md#reconciliation-loop) for operational guidance and [Architecture — `_cockroach_migration_tool`](architecture.md#_cockroach_migration_tool-helper-schema) for diagnostic queries.

### `mappings`

A list of one or more mapping objects. Each mapping ties one source CockroachDB database and set of tables to one destination PostgreSQL connection.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Stable identifier for the `/ingest/<id>` route. Must be unique across all mappings. |
| `source` | object | yes | Source CockroachDB database and tables |
| `destination` | object | yes | Destination PostgreSQL connection |

#### `source`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `database` | string | yes | Source CockroachDB database name |
| `tables` | list of strings | yes | Schema-qualified table names, e.g. `public.customers`. At least one. Unique within the mapping. |

```yaml
source:
  database: demo_a
  tables:
    - public.customers
    - public.orders
```

#### `destination`

Two mutually exclusive forms: **URL** or **decomposed fields**. Never mix them.

##### URL form

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a
```

For TLS, add query parameters:

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt
```

##### Decomposed form

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `host` | string | yes | PostgreSQL hostname or IP |
| `port` | integer | yes | PostgreSQL port |
| `database` | string | yes | Target database name |
| `user` | string | yes | Database user |
| `password` | string | yes | Database password |
| `tls` | object | no | Destination TLS configuration |

> **Production note (secrets):** Plaintext `password` fields in YAML are operationally simple examples. In production, source credentials from your normal secret-management workflow (e.g. a vault or sealed-secrets controller), materialize the final config file with those secrets injected, and ensure it is only readable by the runner process.
```yaml
destination:
  host: pg-a.example.internal
  port: 5432
  database: app_a
  user: migration_user_a
  password: runner-secret-a
```

##### `destination.tls` (decomposed form only)

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `mode` | string | yes | `require`, `verify-ca`, or `verify-full` |
| `ca_cert_path` | path | required for `verify-ca` and `verify-full` | CA certificate for server verification |
| `client_cert_path` | path | no | Client certificate for mTLS. Must appear with `client_key_path`. |
| `client_key_path` | path | no | Client private key. Must appear with `client_cert_path`. |

| `mode` | Server certificate verified | `ca_cert_path` required |
|--------|----------------------------|------------------------|
| `require` | No | No |
| `verify-ca` | Yes (against CA) | Yes |
| `verify-full` | Yes (CA + hostname) | Yes |

```yaml
destination:
  host: pg-a.example.internal
  port: 5432
  database: app_a
  user: migration_user_a
  password: runner-secret-a
  tls:
    mode: verify-full
    ca_cert_path: /config/certs/destination-ca.crt
    client_cert_path: /config/certs/destination-client.crt
    client_key_path: /config/certs/destination-client.key
```

### Full example

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
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
        mode: verify-full
        ca_cert_path: /config/certs/destination-ca.crt
        client_cert_path: /config/certs/destination-client.crt
        client_key_path: /config/certs/destination-client.key
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
    destination:
      url: postgresql://migration_user_b:runner-secret-b@pg-b.example.internal:5432/app_b?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt
```

## HTTP endpoints

All endpoints are served on the address configured in `webhook.bind_addr`.

### Health check

```
GET /healthz
```

Returns `200 OK` when the runner is alive.

```bash
curl -k https://runner.example.internal:8443/healthz
```

### Metrics

```
GET /metrics
```

Returns Prometheus-formatted metrics as `text/plain`. Metric names are prefixed with `cockroach_migration_tool_runner_`.

### Ingest

```
POST /ingest/{mapping_id}
Content-Type: application/json
```

The endpoint CockroachDB changefeeds post to. The `mapping_id` must exactly match a mapping `id` in the runner config.

#### Webhook payload format

Row batch:

```json
{
  "length": 2,
  "payload": [
    {
      "after": {"id": 1, "email": "first@example.com"},
      "key": {"id": 1},
      "op": "c",
      "source": {
        "database_name": "demo_a",
        "schema_name": "public",
        "table_name": "customers"
      }
    },
    {
      "key": {"id": 2},
      "op": "d",
      "source": {
        "database_name": "demo_a",
        "schema_name": "public",
        "table_name": "customers"
      }
    }
  ]
}
```

Resolved watermark:

```json
{"resolved": "1776526353000000000.0000000000"}
```

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `length` | integer | yes | Must equal the number of entries in `payload` |
| `payload` | array | yes | Row events |
| `payload[].source.database_name` | string | yes | Database label |
| `payload[].source.schema_name` | string | yes | Schema label |
| `payload[].source.table_name` | string | yes | Table label |
| `payload[].op` | string | yes | `c` (create), `u` (update), `r` (refresh), `d` (delete) |
| `payload[].key` | object | yes | JSON key-column map |
| `payload[].after` | object | required for `c`, `u`, `r` | JSON post-change column map. Omit for `d`. |
| `resolved` | string | yes (watermark) | Non-empty resolved timestamp |

All events in a single batch must reference the same source table. `key` and `after` are arbitrary JSON column-value maps.

#### Response codes

| Status | Meaning |
|--------|---------|
| `200 OK` | Batch accepted |
| `400 Bad Request` | Malformed batch (e.g. length mismatch) |
| `404 Not Found` | Unknown `mapping_id` |
| `500 Internal Server Error` | Processing failure |

#### Manual test

```bash
curl -k -X POST \
  -H 'content-type: application/json' \
  -d '{"length":1,"payload":[{"after":{"id":1,"name":"test"},"key":{"id":1},"op":"c","source":{"database_name":"demo_a","schema_name":"public","table_name":"customers"}}]}' \
  https://localhost:8443/ingest/app-a
```

## End-to-end walkthrough

### 1. Pull the image

```bash
export GITHUB_OWNER="<your-github-org-or-user>"
export RUNNER_IMAGE="ghcr.io/${GITHUB_OWNER}/runner-image:<git-sha>"
docker pull "${RUNNER_IMAGE}"
```

### 2. Write config

Create `config/runner.yml`. Minimal HTTPS example:

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

Minimal HTTP example (development only):

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

### 3. Validate

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml
```

With destination connectivity check:

```bash
docker run --rm \
  --network host \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml --deep
```

### 4. Start

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  run --config /config/runner.yml
```

With structured logging:

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  --log-format json \
  run --config /config/runner.yml
```

### 5. Verify

```bash
curl -k https://localhost:8443/healthz
```

## Docker Compose

`runner.compose.yml`:

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
      - --log-format
      - json
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

```bash
docker compose -f runner.compose.yml run --rm runner validate-config --config /config/runner.yml
docker compose -f runner.compose.yml up runner
```
