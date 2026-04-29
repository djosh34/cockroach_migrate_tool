# Getting Started

This page walks through the full operator flow — pull images, write configs, start services, and run a verify job — in one place. You will have a working migration pipeline by the end.

## Prerequisites

- **Docker** 20.10+ with access to `ghcr.io`
- **GitHub token** with `read:packages` scope
- **TLS certificates** (PEM-encoded). You need them before configuring anything. See [TLS Configuration](tls-configuration.md).
- A **CockroachDB** source cluster with `kv.rangefeed.enabled = true`
- One or more **PostgreSQL** destination databases, schemas, and tables, all pre-created with a shape compatible with the source (the runner creates only the `_cockroach_migration_tool` helper schema — it does not create databases, schemas, or destination tables)

## Step 1 — Authenticate with GHCR

```bash
echo "$GITHUB_TOKEN" | docker login ghcr.io -u "$GITHUB_USERNAME" --password-stdin
```

Find the correct image path and tag for your deployment. Images are published to `ghcr.io/${GITHUB_OWNER}/runner-image` and `ghcr.io/${GITHUB_OWNER}/verify-image`, tagged by full Git commit SHA. See [Installation](installation.md) for image paths, tag discovery, pull commands, and the Quay mirror — this page assumes you have the image references ready.

```bash
export GITHUB_OWNER="<your-github-org-or-user>"
export RUNNER_IMAGE="ghcr.io/${GITHUB_OWNER}/runner-image:<tag>"   # full 40-char SHA from the package page
export VERIFY_IMAGE="ghcr.io/${GITHUB_OWNER}/verify-image:<tag>"
```

## Step 2 — Prepare TLS certificates

Create a `config/certs/` directory with the certificates your configuration will reference. At minimum the runner webhook listener needs a server certificate and key. For production, add server certificates for the verify-service listener and CA certificates for database connections.

```bash
mkdir -p config/certs
# Place your PEM files under config/certs/
```

See [TLS Configuration](tls-configuration.md) for every TLS field in one place.

## Step 3 — Configure and prepare databases

### Create CockroachDB changefeeds

Enable rangefeeds, capture a cursor per source database, then create one changefeed per mapping:

```sql
-- Enable rangefeeds (once per cluster)
SET CLUSTER SETTING kv.rangefeed.enabled = true;

-- Capture cursor per database
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;

-- Create changefeed (use the cursor value from above)
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders
INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=...'
WITH cursor = '1745877420457561000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';
```

See [Source & Destination Setup](setup-sql.md) for the full procedure including `ca_cert` encoding, a complete worked example, and SQL generator scripts.

### Grant PostgreSQL destination permissions

**Before starting the runner**, your destination databases, schemas, and tables must already exist with a shape compatible with the source. The runner only creates the `_cockroach_migration_tool` helper schema — it does not create databases, schemas, or destination tables.

Run these grants once per destination database. Replace placeholders with your actual role and table names:

```sql
GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;
GRANT USAGE ON SCHEMA public TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.orders TO migration_user_a;
```

See [Source & Destination Setup](setup-sql.md) for the full procedure including SQL generator scripts.

## Step 4 — Write runner config

Create `config/runner.yml`:

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

> **`mode: http` for local development:** The example above uses `mode: https` with TLS certificates — the recommended production path. For a zero-TLS local development workflow, set `webhook.mode: http`, change `bind_addr` to a plain-HTTP port (e.g. `8080`), and omit the entire `webhook.tls` block. HTTPS remains the main example throughout this guide.

> **Production note (secrets):** The `password` fields shown above are plaintext example values for an operationally simple walkthrough. In production, source database credentials from your normal secret-management workflow (e.g. a vault or sealed-secrets controller), materialize the final `runner.yml` with those secrets injected, and ensure the file is only readable by the runner process.

Validate:

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml

# Test destination connectivity
docker run --rm \
  --network host \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml --deep
```

## Step 5 — Start the runner

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  run --config /config/runner.yml
```

Verify it is listening (the `-k` flag skips TLS certificate verification — convenient for self-signed certs in local development, but not for production):

```bash
curl -k https://localhost:8443/healthz
```

The runner immediately bootstraps the `_cockroach_migration_tool` helper schema and begins the reconcile loop. CockroachDB changefeeds can connect now — they retry until the runner is reachable.

## Step 6 — Write verify-service config

Create `config/verify-service.yml`:

```yaml
listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    url: postgresql://verify_source:source-password@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
  destination:
    url: postgresql://verify_target:target-password@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
```

Validate:

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service validate-config --config /config/verify-service.yml
```

## Step 7 — Start the verify-service

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service run --config /config/verify-service.yml
```

## Step 8 — Run a verify job

The example below targets the verify-service on plain HTTP (`http://localhost:8080`) — this is a minimal local example matching the HTTP listener config in Step 6. For production with TLS, use `https://` and a proper hostname.

```bash
export VERIFY_API="http://localhost:8080"

# Start a job verifying all public tables
JOB_ID=$(curl --silent --show-error \
  -H 'content-type: application/json' \
  -d '{"include_schema":"^public$"}' \
  "${VERIFY_API}/jobs" | jq -r '.job_id')

# Poll for completion
curl --silent --show-error \
  "${VERIFY_API}/jobs/${JOB_ID}"
```

**What success looks like** — a job that completed with no mismatches (`status: "succeeded"`, `result.summary.has_mismatches: false`):

```json
{
  "job_id": "cbf8cbb8-...",
  "status": "succeeded",
  "result": {
    "summary": {
      "tables_verified": 4,
      "tables_with_data": 4,
      "has_mismatches": false
    },
    "table_summaries": [
      {
        "schema": "public",
        "table": "customers",
        "num_verified": 7700,
        "num_success": 7700,
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

**What failure looks like** — a job that found mismatches (`status: "failed"`, `result.summary.has_mismatches: true`):

```json
{
  "job_id": "cbf8cbb8-...",
  "status": "failed",
  "failure": {
    "category": "mismatch",
    "code": "mismatch_detected",
    "message": "verify detected mismatches in 1 table",
    "details": [{"reason": "mismatch detected for public.customers"}]
  },
  "result": {
    "summary": {
      "tables_verified": 4,
      "tables_with_data": 4,
      "has_mismatches": true
    },
    "table_summaries": [
      {
        "schema": "public",
        "table": "customers",
        "num_verified": 7700,
        "num_success": 7699,
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
        "table": "customers",
        "primary_key": {"id": "1007"},
        "mismatching_columns": ["email"],
        "source_values": {"email": "old@example.com"},
        "destination_values": {"email": "new@example.com"},
        "info": ["email mismatch"]
      }
    ],
    "mismatch_summary": {
      "has_mismatches": true,
      "affected_tables": [{"schema": "public", "table": "customers"}],
      "counts_by_kind": {"mismatching_column": 1}
    }
  }
}
```

If the job is still running, `status` is `"running"` with no `result` or `failure` fields — continue polling.

The verify-service connects to both databases, discovers tables matching the filters, compares row data, and returns findings. See [Verify-Service](verify-service.md) for the full API.

## What next?

- [Runner config reference](runner.md) — every field and option
- [Verify-Service config reference](verify-service.md) — job lifecycle, error categories, raw table read
- [Architecture](architecture.md) — how the pieces fit together internally
- [Troubleshooting](troubleshooting.md) — common failures and fixes
