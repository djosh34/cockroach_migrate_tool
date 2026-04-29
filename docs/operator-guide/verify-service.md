# Verify-Service

The verify-service image exposes an HTTP API for starting, polling, and stopping verification jobs that compare CockroachDB source data against PostgreSQL destination data row-by-row.

For a deeper explanation of how table discovery, filtering, sharding, and row comparison work internally, see [Architecture — Verify-service internals](architecture.md#verify-service-internals).

## Key constraints

- **Only one job runs at a time.** Starting a second job returns `409 Conflict`.
- **Only the most recent completed job is retained.** Starting a new job evicts the previous result.
- **Job state is in-memory.** All job history is lost on process restart. Previous job IDs return `404 Not Found`.

## Health checking the verify-service

The verify-service does **not** expose a `/healthz` endpoint. To confirm the service is alive use one of:

- **`GET /metrics`** — returns `200 OK` and Prometheus metrics. A non-200 response means the service is not healthy.
- **TCP connect check** — verify the listener port is accepting connections (e.g. `nc -z localhost 8080`).

```bash
# Metrics-based health check
curl --silent --fail http://localhost:8080/metrics > /dev/null && echo "healthy"

# TCP port check
nc -z localhost 8080 && echo "listening"
```

## Quick start

```bash
export GITHUB_OWNER="<your-github-org-or-user>"
export VERIFY_IMAGE="ghcr.io/${GITHUB_OWNER}/verify-image:<git-sha>"
docker pull "${VERIFY_IMAGE}"

docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service run --config /config/verify-service.yml
```

> The verify image entrypoint is `molt` with default command `verify-service`. Always include `verify-service` when overriding `command` in Docker or Compose.

## CLI

```
verify-service validate-config --config <path> [--log-format text|json]
verify-service run --config <path> [--log-format text|json]
```

| Subcommand | Purpose | Flags |
|------------|---------|-------|
| `validate-config` | Check config structure and consistency | `--config <path>` (required) |
| `run` | Start the HTTP listener and accept verify jobs | `--config <path>` (required) |

`--log-format` is a flag on each subcommand, not a global flag. Defaults to `text`.

## Configuration reference

The verify-service reads a single YAML file passed via `--config <path>`.

### Top-level structure

```yaml
listener: ...
verify: ...
```

Both keys are required.

### `listener`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `bind_addr` | string | yes | Host and port, e.g. `0.0.0.0:8080` |
| `tls` | object | no | TLS configuration. Omit for plain HTTP. |

#### `listener.tls`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `cert_path` | path | yes | Server certificate PEM file |
| `key_path` | path | yes | Server private key PEM file |
| `client_ca_path` | path | no | CA certificate for mTLS client verification |

When `tls` is present, `cert_path` and `key_path` are both required. When absent, the listener serves plain HTTP. `client_ca_path` is optional; when set, callers must present a client certificate signed by that CA.

#### Examples

HTTP:

```yaml
listener:
  bind_addr: 0.0.0.0:8080
```

HTTPS:

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

HTTPS with mTLS:

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

### `verify`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `source` | object | no | Default source connection settings shared across database mappings |
| `destination` | object | no | Default destination connection settings shared across database mappings |
| `databases` | list of objects | yes | Named database mappings to verify |
| `raw_table_output` | boolean | no | Enable `POST /tables/raw` endpoint. Defaults to `false`. |

#### Connection blocks

Both use the same shape:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `host` | string | yes after defaults and overrides are merged | Database hostname |
| `port` | integer | yes after defaults and overrides are merged | Database port |
| `database` | string | yes for fully specified per-database blocks | Database name |
| `user` | string | yes after defaults and overrides are merged | Database user |
| `password_file` | path | no | Password file path passed through as libpq `passfile` |
| `sslmode` | string | yes after defaults and overrides are merged | `disable`, `require`, `verify-ca`, or `verify-full` |
| `tls` | object | no | File paths for TLS certificates and keys |

```yaml
verify:
  source:
    host: source.internal
    port: 26257
    user: verify_source
    sslmode: verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    sslmode: verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
  databases:
    - name: app
      source_database: appdb
      destination_database: appdb
```

##### `tls` under source or destination

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `ca_cert_path` | path | required when `sslmode` is `verify-ca` or `verify-full` | CA certificate for server verification |
| `client_cert_path` | path | no | Client certificate for mTLS |
| `client_key_path` | path | no | Client private key. Must appear with `client_cert_path`. |

`sslmode` values:

| `sslmode` | Server verification | Requires `ca_cert_path` |
|-----------|---------------------|------------------------|
| `disable` | No TLS | No |
| `require` | TLS, no verification | No |
| `verify-ca` | TLS, verify against CA | Yes |
| `verify-full` | TLS, verify CA + hostname | Yes |

When `sslmode=verify-ca` or `sslmode=verify-full`, `ca_cert_path` is required. `client_cert_path` and `client_key_path` must always appear as a pair.

#### `verify.databases[]`

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Stable configured database name used by the API |
| `source_database` | string | yes when shared defaults are used | Source database name for this mapping |
| `destination_database` | string | yes when shared defaults are used | Destination database name for this mapping |
| `source` | object | no | Per-database source overrides or a fully specified source block |
| `destination` | object | no | Per-database destination overrides or a fully specified destination block |

Every `verify.databases[]` entry must be an object. Scalar entries such as `- app` are rejected.

### Full examples

Shared defaults; each mapping only names source and destination databases:

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
verify:
  raw_table_output: true
  source:
    host: source.internal
    port: 26257
    user: verify_source
    password_file: /config/secrets/source-password
    sslmode: verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    password_file: /config/secrets/destination-password
    sslmode: verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
  databases:
    - name: app
      source_database: app
      destination_database: app
    - name: billing
      source_database: billing
      destination_database: billing
    - name: support
      source_database: support
      destination_database: support_archive
```

Shared defaults with per-database credential overrides:

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
verify:
  raw_table_output: true
  source:
    host: source.internal
    port: 26257
    user: verify_source
    password_file: /config/secrets/source-password
    sslmode: verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    password_file: /config/secrets/destination-password
    sslmode: verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
  databases:
    - name: app
      source_database: app
      destination_database: app
    - name: audit
      source_database: audit
      destination_database: audit
      source:
        user: verify_audit_source
        password_file: /config/secrets/audit-source-password
      destination:
        user: verify_audit_target
        password_file: /config/secrets/audit-destination-password
```

Fully specified per-database connections without shared defaults:

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
verify:
  raw_table_output: true
  databases:
    - name: app
      source:
        host: source.internal
        port: 26257
        database: app
        user: verify_app_source
        password_file: /config/secrets/app-source-password
        sslmode: verify-full
        tls:
          ca_cert_path: /config/certs/source-ca.crt
          client_cert_path: /config/certs/source-client.crt
          client_key_path: /config/certs/source-client.key
      destination:
        host: destination.internal
        port: 5432
        database: app
        user: verify_app_target
        password_file: /config/secrets/app-destination-password
        sslmode: verify-ca
        tls:
          ca_cert_path: /config/certs/destination-ca.crt
    - name: billing
      source:
        host: source.internal
        port: 26257
        database: billing
        user: verify_billing_source
        password_file: /config/secrets/billing-source-password
        sslmode: verify-full
        tls:
          ca_cert_path: /config/certs/source-ca.crt
          client_cert_path: /config/certs/source-client.crt
          client_key_path: /config/certs/source-client.key
      destination:
        host: destination.internal
        port: 5432
        database: billing_prod
        user: verify_billing_target
        password_file: /config/secrets/billing-destination-password
        sslmode: verify-ca
        tls:
          ca_cert_path: /config/certs/destination-ca.crt
```

## Job lifecycle

### Endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `POST` | `/jobs` | Start a verify job |
| `GET` | `/jobs/{job_id}` | Poll job status |
| `POST` | `/jobs/{job_id}/stop` | Request cancellation |
| `POST` | `/tables/raw` | Read raw table rows |
| `GET` | `/metrics` | Prometheus metrics |

### Start a job

```
POST /jobs
Content-Type: application/json
```

Request body (`database` selects the configured mapping; the filter fields remain optional POSIX regular expressions):

```json
{
  "database": "app",
  "include_schema": "^public$",
  "include_table": "^(accounts|orders)$"
}
```

| Field | Description |
|-------|-------------|
| `database` | Configured database mapping name. Required when more than one mapping is configured. |
| `include_schema` | Include schemas matching this regex |
| `include_table` | Include tables matching this regex |
| `exclude_schema` | Exclude schemas matching this regex |
| `exclude_table` | Exclude tables matching this regex |

To verify everything, send `{}`.

Filters are POSIX regular expressions applied against `pg_class` / `pg_namespace` results. Table discovery excludes system schemas (`pg_catalog`, `information_schema`, `crdb_internal`, `pg_extension`). See [Architecture — How table comparison works](architecture.md#how-table-comparison-works) for the full pipeline.

**Accepted — `202`:**

```json
{"job_id": "job-000001", "status": "running"}
```

**Already running — `409 Conflict`:**

```json
{"error": {"category": "job_state", "code": "job_already_running", "message": "a verify job is already running"}}
```

**Validation error — `400`:**

```json
{"error": {"category": "request_validation", "code": "invalid_database_selection", "message": "request validation failed", "details": [{"field": "database", "reason": "database selection is required when multiple databases are configured"}]}}
```

### Poll job status

```
GET /jobs/{job_id}
```

Poll every 2 seconds until status is no longer `running` or `stopping`.

**Running — `200 OK`:**

```json
{"job_id": "job-000001", "status": "running"}
```

**Succeeded — `200 OK`:**

```json
{
  "job_id": "job-000001",
  "status": "succeeded",
  "result": {
    "summary": {
      "tables_verified": 1,
      "tables_with_data": 1,
      "has_mismatches": false
    },
    "table_summaries": [
      {
        "schema": "public",
        "table": "accounts",
        "num_verified": 7,
        "num_success": 7,
        "num_missing": 0,
        "num_mismatch": 0,
        "num_column_mismatch": 0,
        "num_extraneous": 0,
        "num_live_retry": 0
      }
    ],
    "findings": [],
    "mismatch_summary": {
      "has_mismatches": false,
      "affected_tables": [],
      "counts_by_kind": {}
    }
  }
}
```

**Failed with mismatches — `200 OK`:**

```json
{
  "job_id": "job-000001",
  "status": "failed",
  "failure": {
    "category": "mismatch",
    "code": "mismatch_detected",
    "message": "verify detected mismatches in 1 table",
    "details": [{"reason": "mismatch detected for public.accounts"}]
  },
  "result": {
    "summary": {
      "tables_verified": 1,
      "tables_with_data": 1,
      "has_mismatches": true
    },
    "table_summaries": [
      {
        "schema": "public",
        "table": "accounts",
        "num_verified": 7,
        "num_success": 6,
        "num_missing": 0,
        "num_mismatch": 0,
        "num_column_mismatch": 1,
        "num_extraneous": 0,
        "num_live_retry": 0
      }
    ],
    "findings": [
      {
        "kind": "mismatching_column",
        "schema": "public",
        "table": "accounts",
        "primary_key": {"id": "101"},
        "mismatching_columns": ["balance"],
        "source_values": {"balance": "17"},
        "destination_values": {"balance": "23"},
        "info": ["balance mismatch"]
      }
    ],
    "mismatch_summary": {
      "has_mismatches": true,
      "affected_tables": [{"schema": "public", "table": "accounts"}],
      "counts_by_kind": {"mismatching_column": 1}
    }
  }
}
```

**Failed with connection error — `200 OK`:**

```json
{
  "job_id": "job-000001",
  "status": "failed",
  "failure": {
    "category": "source_access",
    "code": "connection_failed",
    "message": "source connection failed: dial tcp 127.0.0.1:5432: connect: connection refused",
    "details": [{"reason": "dial tcp 127.0.0.1:5432: connect: connection refused"}]
  }
}
```

**Not found — `404`:**

```json
{"error": {"category": "job_state", "code": "job_not_found", "message": "job not found"}}
```

### Stop a job

```
POST /jobs/{job_id}/stop
Content-Type: application/json

{}
```

Immediate response — `200 OK`:

```json
{"job_id": "job-000001", "status": "stopping"}
```

The job transitions to `stopped` asynchronously. Poll until status is `stopped`.

### Metrics

```
GET /metrics
```

Returns Prometheus-formatted metrics as `text/plain`. Metric names are prefixed with `cockroach_migration_tool_verify_`.

### Raw table read

```
POST /tables/raw
Content-Type: application/json

{"database": "app", "side": "source", "schema": "public", "table": "accounts"}
```

Only available when `verify.raw_table_output` is `true`. Returns `403` if disabled.

### Job states

| Status | Meaning | Terminal |
|--------|---------|----------|
| `running` | Job is actively verifying | No |
| `stopping` | Stop requested, winding down | No |
| `succeeded` | Verification completed, no mismatches | Yes |
| `failed` | Completed with mismatches or error | Yes |
| `stopped` | Cancelled by operator | Yes |

### Error categories

| Category | When it occurs |
|----------|---------------|
| `request_validation` | Invalid filter, unknown field, body too large |
| `job_state` | Job already running, job not found |
| `source_access` | Cannot connect to source database |
| `destination_access` | Cannot connect to destination database |
| `mismatch` | Mismatches detected during verification |
| `verify_execution` | Internal verify execution failure |

### Interpreting results

1. Check `result.summary.has_mismatches`.
2. If `true`, inspect `result.mismatch_summary.affected_tables`.
3. For per-row detail, check `result.findings` — each finding includes `mismatching_columns`, `source_values`, and `destination_values`.

## End-to-end walkthrough

### 1. Pull the image

```bash
export GITHUB_OWNER="<your-github-org-or-user>"
export VERIFY_IMAGE="ghcr.io/${GITHUB_OWNER}/verify-image:<git-sha>"
docker pull "${VERIFY_IMAGE}"
```

### 2. Write config

Create `config/verify-service.yml`. Minimal HTTP example:

```yaml
listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    host: source.internal
    port: 26257
    user: verify_source
    password_file: /config/secrets/source-password
    sslmode: verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    password_file: /config/secrets/destination-password
    sslmode: verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
  databases:
    - name: app
      source_database: appdb
      destination_database: appdb
```

HTTPS with mTLS:

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
verify:
  source:
    host: source.internal
    port: 26257
    user: verify_source
    sslmode: verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    host: destination.internal
    port: 5432
    user: verify_target
    password_file: /config/secrets/destination-password
    sslmode: verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
  databases:
    - name: app
      source_database: appdb
      destination_database: appdb
```

### 3. Validate

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service validate-config --config /config/verify-service.yml
```

### 4. Start

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service run --config /config/verify-service.yml
```

With structured logging:

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service run --log-format json --config /config/verify-service.yml
```

### 5. Run a verify job

```bash
export VERIFY_API="http://localhost:8080"

# Start
JOB_ID=$(curl --silent --show-error \
  -H 'content-type: application/json' \
  -d '{"include_schema":"^public$","include_table":"^(accounts|orders)$"}' \
  "${VERIFY_API}/jobs" | jq -r '.job_id')

# Poll
curl --silent --show-error \
  "${VERIFY_API}/jobs/${JOB_ID}"

# Stop if needed
curl --silent --show-error \
  -H 'content-type: application/json' \
  -d '{}' \
  -X POST "${VERIFY_API}/jobs/${JOB_ID}/stop"
```

## Docker Compose

`verify.compose.yml`:

```yaml
services:
  verify:
    image: "${VERIFY_IMAGE}"
    network_mode: bridge
    ports:
      - "${VERIFY_HTTPS_PORT:-9443}:8443"
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
      - verify-service
      - run
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

```bash
docker compose -f verify.compose.yml up verify
```
