# Cockroach Migrate Tool

Run the published `runner` and `verify` images with inline configs.

Before you start the runtime, prepare your CockroachDB changefeeds and destination PostgreSQL grants with operator-managed SQL for your environment. This repository no longer ships a dedicated SQL-emitter binary or compose artifact.

## Runner Quick Start

Pull the runner image and write config. The example uses HTTP on `8080`; for HTTPS, set `mode: https` and mount `webhook.tls.cert_path` plus `webhook.tls.key_path`.

`mappings[].source` rejects misrouted payloads.

```bash
export GITHUB_OWNER=<github-owner>
export IMAGE_TAG=<published-commit-sha>
export RUNNER_IMAGE="ghcr.io/${GITHUB_OWNER}/cockroach-migrate-runner:${IMAGE_TAG}"
docker pull "${RUNNER_IMAGE}"
```

```yaml
# config/runner.yml
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
        - public.orders
    destination:
      url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a
```

For HTTPS, switch to:

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

For TLS targets, add `sslmode=verify-ca`, `sslrootcert=/config/certs/destination-ca.crt`, `sslcert=/config/certs/destination-client.crt`, and `sslkey=/config/certs/destination-client.key` query params. TLS reference: `docs/tls-configuration.md`.

Explicit-field alternative:

```yaml
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

Validate:

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml
```

Plain `validate-config` stays offline. Add `--deep` to verify destination connectivity and mapped tables.

Required args:

- `validate-config --config /config/runner.yml`
- `run --config /config/runner.yml`

Optional args:

- `--log-format json` for structured stderr logs
- `--deep` to verify destination connectivity and mapped tables

Start the runtime after your source changefeeds and destination grants are already in place:

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  run --config /config/runner.yml
```

- `GET /healthz`
- `POST /ingest/<mapping_id>`

### Webhook Payload Format

```json
{"length":2,"payload":[{"after":{"id":1,"email":"first@example.com"},"key":{"id":1},"op":"c","source":{"database_name":"demo_a","schema_name":"public","table_name":"customers"}},{"key":{"id":2},"op":"d","source":{"database_name":"demo_a","schema_name":"public","table_name":"customers"}}]}
```

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `length` | integer | required | `payload` size. |
| `payload` | array | required | Row events. |
| `payload[]` | object | required | One event. |
| `payload[].source.database_name` | string | required | Database label. |
| `payload[].source.schema_name` | string | required | Schema label. |
| `payload[].source.table_name` | string | required | Table label. |
| `payload[].op` | string | required | `c` create/insert, `u` update, `r` refresh/upsert, `d` delete. |
| `payload[].key` | object | required | JSON key-column map. |
| `payload[].after` | object | required for `c`, `u`, `r` | JSON post-change column map; omit for `d`. |
| `resolved` | string | required for resolved | Non-empty watermark. |

- `length` must equal the number of entries in `payload`.
- Every event in one batch must use the same `source` table.

```json
{"resolved":"1776526353000000000.0000000000"}
```

`key` and `after` are arbitrary JSON column maps.

```bash
curl -H 'content-type: application/json' --data '{"length":2,"payload":[{"after":{"id":1,"email":"first@example.com"},"key":{"id":1},"op":"c","source":{"database_name":"demo_a","schema_name":"public","table_name":"customers"}},{"key":{"id":2},"op":"d","source":{"database_name":"demo_a","schema_name":"public","table_name":"customers"}}]}' http://127.0.0.1:8080/ingest/app-a
```

- `200 OK`
- `400 Bad Request`: ``row-batch request `length` must match payload size``
- `404 Unknown Mapping`
- `500 Internal Server Error`

Save this as `runner.compose.yml`:

```yaml
services:
  runner:
    image: "${RUNNER_IMAGE}"
    network_mode: bridge
    ports:
      - "${RUNNER_HTTP_PORT:-8080}:8080"
    configs:
      - source: runner-config
        target: /config/runner.yml
    command:
      - run
      - --config
      - /config/runner.yml

configs:
  runner-config:
    file: ./config/runner.yml
```

```bash
docker compose -f runner.compose.yml run --rm runner validate-config --config /config/runner.yml
docker compose -f runner.compose.yml up runner
```

## Verify Quick Start

Pull verify image and write config. Use `listener.bind_addr` for HTTP, `cert_path` plus `key_path` for HTTPS, `client_ca_path` for mTLS, and `sslmode` in DB URLs. TLS reference: `docs/tls-configuration.md`.
`openapi/verify-service.yaml`.

```bash
export GITHUB_OWNER=<github-owner>
export IMAGE_TAG=<published-commit-sha>
export VERIFY_IMAGE="ghcr.io/${GITHUB_OWNER}/cockroach-migrate-verify:${IMAGE_TAG}"
docker pull "${VERIFY_IMAGE}"
mkdir -p config/certs
```

```yaml
# config/verify-service.yml
listener:
  bind_addr: 0.0.0.0:8080
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    url: postgresql://verify_target@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
```

Validate the mounted config through the image entrypoint:

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  validate-config --log-format json --config /config/verify-service.yml
```

Start the verify API through the image entrypoint:

```bash
docker run --rm \
  -p 9443:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  run --log-format json --config /config/verify-service.yml
```

Required args:

- `validate-config --config /config/verify-service.yml`
- `run --config /config/verify-service.yml`

Optional args:

- `--log-format json` for structured stderr logs

### Job Lifecycle

- `running`: actively verifying
- `succeeded`: verification completed with no mismatches
- `failed`: verification completed with mismatches or encountered an error
- `stopped`: explicitly cancelled via `POST /jobs/{job_id}/stop`

Poll `GET /jobs/{job_id}` every 2 seconds until `status` is no longer `running`.
Only one job can run at a time. Starting a second job returns `HTTP 409 Conflict`.

```json
{"error":{"category":"job_state","code":"job_already_running","message":"a verify job is already running"}}
```

Only the most recent completed job is retained. Starting a new job evicts the previous completed job. Job state is held in memory. If the verify service process restarts, previous job IDs return `HTTP 404`.

```bash
export VERIFY_API="https://localhost:9443"
```

Start:

```bash
curl --silent --show-error --insecure \
  --cert config/certs/source-client.crt \
  --key config/certs/source-client.key \
  -H 'content-type: application/json' \
  -d '{"include_schema":"^public$","include_table":"^(accounts|orders)$"}' \
  "${VERIFY_API}/jobs"
```

Accepted:

```json
{"job_id":"job-000001","status":"running"}
```

Poll:

```bash
curl --silent --show-error --insecure \
  --cert config/certs/source-client.crt \
  --key config/certs/source-client.key \
  "${VERIFY_API}/jobs/${JOB_ID}"
```

Running:

```json
{"job_id":"job-000001","status":"running"}
```

Succeeded:

```json
{"job_id":"job-000001","status":"succeeded","result":{"summary":{"tables_verified":1,"tables_with_data":1,"has_mismatches":false},"table_summaries":[{"schema":"public","table":"accounts","num_verified":7,"num_success":7,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0}],"findings":[],"mismatch_summary":{"has_mismatches":false,"affected_tables":[],"counts_by_kind":{}}}}
```

Check `result.summary` first, then `result.mismatch_summary`, then `result.findings`.

`POST /jobs/${JOB_ID}/stop` first returns `{"job_id":"job-000001","status":"stopping"}` before terminal `stopped`.
Validation errors: `{"error":{"category":"request_validation","code":"unknown_field",...}}`.
Source failures: `{"job_id":"job-000001","status":"failed","failure":{"category":"source_access","code":"connection_failed",...}}`.
Mismatch completion: `{"job_id":"job-000001","status":"failed","failure":{"category":"mismatch","code":"mismatch_detected",...}}`.

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
      - run
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

Compose API:

```bash
docker compose -f verify.compose.yml up verify
```
