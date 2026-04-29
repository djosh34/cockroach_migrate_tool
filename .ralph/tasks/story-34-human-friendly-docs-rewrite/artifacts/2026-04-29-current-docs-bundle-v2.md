# README.md

# CockroachDB to PostgreSQL Migration Tool

Continuously migrates data from CockroachDB to PostgreSQL using changefeed webhooks, with built-in row-level data verification.

The **runner** receives changefeed batches and writes row mutations into PostgreSQL destination tables. The **verify-service** compares source and destination data to confirm migration correctness.

## Documentation

See the **[Operator Guide](docs/operator-guide/index.md)** for installation, configuration, TLS setup, database preparation, architecture, and troubleshooting.

## Development

Use the repository flake for local development:

```bash
nix run .#check
nix run .#lint
nix run .#test
nix develop
```


# docs/operator-guide/index.md

# CockroachDB to PostgreSQL Migration Tool — Operator Guide

Two published container images — `runner-image` and `verify-image` — together form a complete CockroachDB-to-PostgreSQL migration pipeline.

## What problem does this solve?

You have data in CockroachDB that must move into PostgreSQL, and the migration cannot be a one-time snapshot. CockroachDB changefeeds emit every row mutation as a real-time stream. This tool receives those streams, writes them into PostgreSQL destination tables, and lets you verify that every row arrived correctly.

## How the pieces fit together

```
CockroachDB source
      │
      │  changefeeds push row batches
      ▼
┌──────────┐     ┌────────────────┐
│  runner   │────▶│  PostgreSQL    │
│  image    │     │  destination   │
└──────────┘     └────────────────┘
                      │
                      │  verify-service
                      │  reads both sides
                      ▼
               ┌────────────────┐
               │  verify-image   │
               │  checks every   │
               │  row matches    │
               └────────────────┘
```

| Image | Role | Registry |
|-------|------|----------|
| `runner-image` | Receives changefeed webhooks, writes rows into PostgreSQL | GHCR (primary), Quay (mirror) |
| `verify-image` | Compares source and destination data row-by-row | GHCR (primary), Quay (mirror) |

Both images are multi-platform (`linux/amd64`, `linux/arm64`) and tagged by full Git commit SHA.

## Operator workflow

1. **[Getting Started](getting-started.md)** — One-page happy-path walkthrough covering the entire flow.
2. **[Install the images](installation.md)** — Pull from GHCR, authenticate, understand tags.
3. **[Set up TLS certificates](tls-configuration.md)** — Certificates are required before writing component configs. Every TLS field across all components in one place.
4. **[Review the configuration reference](config-reference.md)** — Single-page hub for all config files, common operator decisions, and where to find field-level details. Read this before writing runner or verify-service configs.
5. **[Configure CockroachDB and PostgreSQL](setup-sql.md)** — Enable rangefeeds, create changefeeds, grant destination permissions.
6. **[Configure and run the runner](runner.md)** — Write YAML config, validate, start the webhook listener.
7. **[Configure and run the verify-service](verify-service.md)** — Write YAML config, validate, start the API, run and poll verify jobs.
8. **[Understand the architecture](architecture.md)** — Internals: webhook ingestion, reconciliation, helper schema, table comparison.
9. **[Troubleshoot](troubleshooting.md)** — Diagnose common failures.

**Order matters:** TLS certificates must exist before writing runner or verify-service configs (configs reference certificate paths). CockroachDB changefeeds and PostgreSQL grants must be in place before the runner starts. CockroachDB retries webhook deliveries, so changefeeds can be created before the runner is reachable — but no data flows until the runner is listening.

## Pages

| Page | Covers |
|------|--------|
| [Getting Started](getting-started.md) | Complete end-to-end walkthrough in one page |
| [Installation](installation.md) | Pull commands, tags, GHCR/Quay, authentication, running containers, log format |
| [TLS Configuration](tls-configuration.md) | TLS settings for runner listener, runner destinations, verify listener, verify database connections |
| [Configuration Reference](config-reference.md) | Single-page hub for all config files: runner, verify-service, TLS, common operator decisions, and where to find field-level details |
| [Source & Destination Setup](setup-sql.md) | CockroachDB changefeeds, PostgreSQL grants, SQL generator scripts |
| [Runner](runner.md) | CLI, configuration reference, HTTP endpoints, webhook payload format, Docker Compose |
| [Verify-Service](verify-service.md) | CLI, configuration reference, job lifecycle API, Docker Compose |
| [Architecture](architecture.md) | Webhook ingestion, reconciliation loop, `_cockroach_migration_tool` helper schema, table comparison internals |
| [Troubleshooting](troubleshooting.md) | Common failures and diagnostic steps |


# docs/operator-guide/installation.md

# Installation

Both images are published to the GitHub Container Registry (GHCR) on every push and mirrored to Quay. GHCR is the source of truth.

## Finding the right image tag

Image tags are the full 40-character Git commit SHA from each push. The GHCR package page for each image lists all published tags.

Set `GITHUB_OWNER` to your GitHub repository owner (the organization or user that owns the repository) before running any commands below, or substitute it directly.

**GHCR paths:**

| Image | GHCR path |
|-------|-----------|
| Runner | `ghcr.io/${GITHUB_OWNER}/runner-image` |
| Verify | `ghcr.io/${GITHUB_OWNER}/verify-image` |

To find the latest tag, visit your repository's package pages:

- Runner: `https://github.com/${GITHUB_OWNER}/<repo>/pkgs/container/runner-image`
- Verify: `https://github.com/${GITHUB_OWNER}/<repo>/pkgs/container/verify-image`

Each page shows available tag versions (commit SHAs) and multi-platform digests. Pick the SHA for the commit you want to deploy.

## Pull commands

```bash
export GITHUB_OWNER="<your-github-org-or-user>"
docker pull "ghcr.io/${GITHUB_OWNER}/runner-image:<git-sha>"
docker pull "ghcr.io/${GITHUB_OWNER}/verify-image:<git-sha>"
```

Replace `<git-sha>` with a full 40-character commit SHA from the package page.

## Quay mirror

Images are copied to Quay after each GHCR publish. Quay repository names are determined at build time and may differ from the GHCR names. GHCR is the source of truth — always determine availability from GHCR, not Quay.

```
quay.io/<quay-organization>/<runner-repository>:<git-sha>
quay.io/<quay-organization>/<verify-repository>:<git-sha>
```

## Authentication

```bash
echo "$GITHUB_TOKEN" | docker login ghcr.io -u "$GITHUB_USERNAME" --password-stdin
```

The token requires the `read:packages` scope.

## Running a container

Both images default to their respective subcommand. Pass arguments after the image name.

### Validate runner config (offline)

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/${GITHUB_OWNER}/runner-image:<git-sha> \
  validate-config --config /config/runner.yml
```

### Validate runner config (deep — tests destination connectivity)

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  --network host \
  ghcr.io/${GITHUB_OWNER}/runner-image:<git-sha> \
  validate-config --config /config/runner.yml --deep
```

### Start the runner

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/${GITHUB_OWNER}/runner-image:<git-sha> \
  run --config /config/runner.yml
```

### Validate verify-service config

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/${GITHUB_OWNER}/verify-image:<git-sha> \
  verify-service validate-config --config /config/verify-service.yml
```

### Start the verify-service

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/${GITHUB_OWNER}/verify-image:<git-sha> \
  verify-service run --config /config/verify-service.yml
```

> **Entrypoint asymmetry.** The runner and verify images use different entrypoint conventions:
>
> | Image | Entrypoint | Default command | How to override `command` in Compose |
> |-------|-----------|----------------|--------------------------------------|
> | `runner-image` | `runner` (the binary) | *(none — default CMD invokes `runner`)* | Pass positional args directly, e.g. `command: ["run", "--config", "/config/runner.yml"]` |
> | `verify-image` | `molt` | `verify-service` | Always include `verify-service` as the first argument, e.g. `command: ["verify-service", "run", "--config", "/config/verify-service.yml"]` |
>
> This explains why CLI examples treat the two images differently: runner commands start directly with a subcommand (`validate-config`, `run`), while verify commands require the `verify-service` prefix (`verify-service validate-config`, `verify-service run`).

## Log format

Both images support `--log-format text|json`. The flag position differs:

| Image | Flag position | Example |
|-------|--------------|---------|
| `runner-image` | Global flag, before the subcommand | `--log-format json validate-config --config ...` |
| `verify-image` | Flag on the subcommand | `verify-service validate-config --log-format json --config ...` |

## Next steps

- [Set up TLS certificates](tls-configuration.md) — required before writing component configs
- [Getting Started](getting-started.md) — complete end-to-end walkthrough
- [Source & Destination Setup](setup-sql.md) — CockroachDB changefeeds and PostgreSQL grants


# docs/operator-guide/getting-started.md

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


# docs/operator-guide/config-reference.md

# Configuration Reference

Everything that can be configured across the migration tool, in one place. This page is the hub — it tells you what configuration exists, which file controls it, and where the full field-level reference lives.

## Configuration files at a glance

| File | Controlled by | What it configures |
|------|--------------|-------------------|
| `config/runner.yml` | `runner-image` (via `--config`) | Webhook listener, reconcile timer, source-to-destination table mappings |
| `config/verify-service.yml` | `verify-image` (via `--config`) | HTTP listener, source and destination database connections for verification |
| Certificate files under `config/certs/` | Both images | TLS identities — server certs, client certs, CA bundles |

Both component configs reference certificate paths under the container mount point `/config/certs/`. Certificates must exist before writing component configs.

## Runner configuration (`config/runner.yml`)

Supplied to the runner via `--config /config/runner.yml`. Full field-by-field reference is in [Runner](runner.md).

### Overall shape

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:                          # required when mode: https
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt   # optional (mTLS)
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
      tls:                      # optional
        mode: verify-full
        ca_cert_path: /config/certs/destination-ca.crt
        client_cert_path: /config/certs/destination-client.crt   # optional (mTLS)
        client_key_path: /config/certs/destination-client.key    # optional (mTLS)
```

### Top-level fields

| Key | Type | Required | Default | Purpose |
|-----|------|----------|---------|---------|
| `webhook` | object | yes | — | HTTPS/HTTP listener that receives changefeed batches |
| `reconcile` | object | yes | — | How often reconciliation copies shadow-table rows into real tables |
| `mappings` | list of objects | yes | — | Source-database → destination-database table mappings (at least one) |

### `webhook` (listener)

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `bind_addr` | string | yes | — | `host:port`, e.g. `0.0.0.0:8443` |
| `mode` | string | no | `https` | `http` or `https` |
| `tls` | object | yes for `mode: https` | — | Must be present for HTTPS, must be absent for HTTP |

#### `webhook.tls`

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `cert_path` | path | yes | — | Server certificate PEM path |
| `key_path` | path | yes | — | Server private key PEM path |
| `client_ca_path` | path | no | — | CA for mTLS client verification |

### `reconcile`

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `interval_secs` | integer | yes | — | Seconds between reconciliation passes. Must be > 0. |

### `mappings[]` (one per source→destination pair)

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `id` | string | yes | — | Stable identifier (unique across mappings), used in `/ingest/<id>` |
| `source` | object | yes | — | Source CockroachDB database and tables |
| `destination` | object | yes | — | Destination PostgreSQL connection |

#### `mappings[].source`

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `database` | string | yes | — | Source CockroachDB database name |
| `tables` | list of strings | yes | — | Schema-qualified table names, e.g. `public.customers`. At least one. |

#### `mappings[].destination`

Two mutually exclusive forms — never mix them in a single destination block.

| Form | Field | Type | Required | Default | Purpose |
|------|-------|------|----------|---------|---------|
| URL | `url` | string | yes | — | Full `postgresql://` connection string |
| Decomposed | `host` | string | yes | — | PostgreSQL hostname or IP |
| Decomposed | `port` | integer | yes | — | PostgreSQL port |
| Decomposed | `database` | string | yes | — | Target database name |
| Decomposed | `user` | string | yes | — | Database user |
| Decomposed | `password` | string | yes | — | Database password |
| Decomposed | `tls` | object | no | — | TLS config (decomposed form only) |

##### `destination.tls` (decomposed form only)

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `mode` | string | yes | — | `require`, `verify-ca`, or `verify-full` |
| `ca_cert_path` | path | required for `verify-ca` and `verify-full` | — | CA certificate for server verification |
| `client_cert_path` | path | no | — | Client certificate for mTLS (must pair with `client_key_path`) |
| `client_key_path` | path | no | — | Client private key (must pair with `client_cert_path`) |

### Common operator decisions

**URL vs decomposed form for destinations.** Choose the URL form when you want to pass the entire connection string as one value (simpler, fewer keys). Choose the decomposed form when you need explicit control over each field or when your environment feeds values from vault/secret-managers field by field. Never mix the two.

**Reconcile interval.** Controls how often shadow tables are flushed into real destination tables. Lower values reduce lag between webhook ingestion and real-table convergence. Higher values give the destination database more breathing room between bulk upsert passes. During bulk initial scans (millions of rows from `initial_scan = 'yes'`), longer intervals reduce destination load. Default recommendation: 30 seconds. See [Architecture](architecture.md) for the reconcile loop details.

**Number of mappings.** One mapping per source database that feeds into a distinct destination database and role. If two source databases share the same destination connection, use two mappings with the same destination config. If one source database has tables going to different destinations, use separate mappings.

## Verify-service configuration (`config/verify-service.yml`)

Supplied to the verify-service via `--config /config/verify-service.yml`. Full field-by-field reference is in [Verify-Service](verify-service.md).

### Overall shape

```yaml
listener:
  bind_addr: 0.0.0.0:8080
  tls:                          # optional (HTTPS); omit for plain HTTP
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt   # optional (mTLS)
verify:
  raw_table_output: false       # optional, defaults to false
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:                        # optional
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt   # optional (mTLS)
      client_key_path: /config/certs/source-client.key     # optional (mTLS)
  destination:
    url: postgresql://verify_target@destination.internal:5432/appdb?sslmode=verify-ca
    tls:                        # optional
      ca_cert_path: /config/certs/destination-ca.crt
```

### Top-level fields

| Key | Type | Required | Default | Purpose |
|-----|------|----------|---------|---------|
| `listener` | object | yes | — | HTTP(S) listener for the job API and metrics |
| `verify` | object | yes | — | Source and destination database connections for row-level comparison |

### `listener`

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `bind_addr` | string | yes | — | `host:port`, e.g. `0.0.0.0:8080` |
| `tls` | object | no | — | TLS configuration. Omit for plain HTTP. |

#### `listener.tls`

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `cert_path` | path | yes (when `tls` present) | — | Server certificate PEM path |
| `key_path` | path | yes (when `tls` present) | — | Server private key PEM path |
| `client_ca_path` | path | no | — | CA for mTLS client verification |

### `verify`

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `source` | object | yes | — | Source (CockroachDB) database connection |
| `destination` | object | yes | — | Destination PostgreSQL database connection |
| `raw_table_output` | boolean | no | `false` | Enable `POST /tables/raw` for diagnostic row reads |

#### `verify.source` and `verify.destination`

Both use the same shape:

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `url` | string | yes | — | Connection URL. Scheme must be `postgresql://` or `postgres://`. Include `sslmode` as a query parameter. |
| `tls` | object | no | — | Certificate file paths for TLS |

##### `tls` under source or destination

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `ca_cert_path` | path | required when `sslmode` is `verify-ca` or `verify-full` | — | CA certificate for server verification |
| `client_cert_path` | path | no | — | Client certificate for mTLS (must pair with `client_key_path`) |
| `client_key_path` | path | no | — | Client private key (must pair with `client_cert_path`) |

##### `sslmode` values

| Value | TLS | Server verification | Requires `ca_cert_path` |
|-------|-----|---------------------|------------------------|
| `disable` | No | — | No |
| `require` | Yes | No | No |
| `verify-ca` | Yes | Against CA | Yes |
| `verify-full` | Yes | CA + hostname | Yes |

### Common operator decisions

**Source URL choice.** The verify-service connects to CockroachDB natively via the PostgreSQL wire protocol. Use a `postgresql://` URL pointing at the CockroachDB cluster. For production, use `sslmode=verify-full` with a CA certificate.

**`raw_table_output`.** Enable `verify.raw_table_output: true` to allow raw row reads via `POST /tables/raw`. This is useful for diagnostics but exposes table contents to any caller that can reach the verify-service API. Disabled by default.

**Job filters.** When starting a verify job (`POST /jobs`), passing `{}` verifies all user tables on both sides. Use `include_schema`, `include_table`, `exclude_schema`, `exclude_table` as POSIX regexes to narrow the scope. All four are optional.

## TLS configuration

For the full TLS field reference, examples, and certificate generation guidance, see [TLS Configuration](tls-configuration.md). That page is the single source for every TLS field across both components.

**When to use TLS:** The runner webhook listener should always use HTTPS in production. CockroachDB changefeeds push data over the network — plain HTTP exposes row data. For database connections, use `verify-ca` or `verify-full` when connecting over untrusted networks. Use `require` only when the network layer already provides integrity (e.g. private VPC with mutual trust). Use `disable` for local development only.

**When to use mTLS:** Enable mTLS on the webhook listener (`webhook.tls.client_ca_path`) to restrict which CockroachDB clusters can push data. Enable mTLS on database connections (`client_cert_path` + `client_key_path`) for passwordless certificate-based authentication.

## Certificate mounting convention

See [TLS Configuration — Certificate mounting convention](tls-configuration.md#certificate-mounting-convention) for the canonical reference. Mount PEM certificates under `/config/certs/` and reference them from config files by those container paths.

## Config validation

Both images include a `validate-config` subcommand. Always validate before running:

```bash
# Runner — offline validation
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml

# Runner — deep validation (tests destination connectivity)
docker run --rm \
  --network host \
  -v "$(pwd)/config:/config:ro" \
  "${RUNNER_IMAGE}" \
  validate-config --config /config/runner.yml --deep

# Verify-service
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service validate-config --config /config/verify-service.yml
```

The `--deep` flag on `runner validate-config` additionally verifies each destination database is reachable and every mapped table exists. Requires network access to the destination databases.

## Log format

Both images support `--log-format text|json`. The flag position differs between images — see [Installation — Log format](installation.md#log-format) for the full reference and examples.

## Summary: where to find config details

| If you need | Go to |
|-------------|-------|
| Every runner config field, with types and descriptions | [Runner](runner.md) |
| Every verify-service config field, with types and descriptions | [Verify-Service](verify-service.md) |
| Every TLS field across all components | [TLS Configuration](tls-configuration.md) |
| Operational guidance on reconcile interval | [Architecture](architecture.md) |
| CockroachDB changefeed setup and PostgreSQL grants | [Source & Destination Setup](setup-sql.md) |
| End-to-end walkthrough that wires everything together | [Getting Started](getting-started.md) |


# docs/operator-guide/tls-configuration.md

# TLS Configuration

Every TLS setting across the runner and verify-service, in one place. Use this page when configuring HTTPS listeners, mTLS, or database connections with certificate verification.

**Do this early.** TLS certificates must exist before you write runner or verify-service configs — both component configs reference certificate paths under `/config/certs/`. Generate and place certificates before proceeding to [Runner](runner.md) or [Verify-Service](verify-service.md).

## Certificate mounting convention

Mount PEM-encoded certificates and keys under `/config/certs/...` inside containers. Config file paths reference these mount points:

```
/config/certs/server.crt
/config/certs/server.key
/config/certs/client-ca.crt
/config/certs/destination-ca.crt
/config/certs/destination-client.crt
/config/certs/destination-client.key
/config/certs/source-ca.crt
/config/certs/source-client.crt
/config/certs/source-client.key
```

## Dev-only: generate self-signed certificates for local testing

> **Local testing only.** These `openssl` commands produce certificates that are not trusted by any public CA. Use a proper PKI (cert-manager, Vault, internal CA) for production.

```bash
# Create a directory for dev certs
mkdir -p config/certs

# Generate a self-signed server certificate (valid 365 days, no passphrase)
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout config/certs/server.key \
  -out config/certs/server.crt \
  -days 365 \
  -subj "/CN=localhost" \
  -addext "subjectAltName=DNS:localhost,IP:127.0.0.1"

# Generate a CA and sign a client certificate for mTLS testing
openssl req -x509 -newkey rsa:2048 -nodes \
  -keyout config/certs/client-ca.key \
  -out config/certs/client-ca.crt \
  -days 365 \
  -subj "/CN=DevClientCA"

# For database connection CA (e.g. when CockroachDB/Postgres also uses self-signed certs),
# copy the database server's CA certificate to config/certs/destination-ca.crt or
# config/certs/source-ca.crt.
```

When using self-signed certs, set `sslmode=verify-ca` or `sslmode=verify-full` and point `ca_cert_path` at the matching CA. Use `curl -k` (skip verification) for quick smoke tests against self-signed HTTPS listeners.

## Runner: webhook listener

| Field | Purpose |
|-------|---------|
| `webhook.mode` | `http` or `https` (default `https`) |
| `webhook.tls.cert_path` | Server certificate |
| `webhook.tls.key_path` | Server private key |
| `webhook.tls.client_ca_path` | CA for mTLS (optional) |

### HTTP (development only)

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
```

No TLS block allowed. Only suitable for trusted local networks.

### HTTPS

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

### HTTPS with mTLS

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

The server verifies that connecting clients present a certificate signed by `client-ca.crt`.

## Runner: destination connection

Two mutually exclusive forms.

### URL form

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt&sslcert=/config/certs/destination-client.crt&sslkey=/config/certs/destination-client.key
```

| `sslmode` | Server verification |
|-----------|---------------------|
| `disable` | No TLS |
| `require` | TLS, no verification |
| `verify-ca` | Verify against CA |
| `verify-full` | Verify CA + hostname |

### Decomposed form with explicit `tls` block

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

| `mode` | Server verification | `ca_cert_path` required |
|--------|---------------------|------------------------|
| `require` | TLS, no verification | No |
| `verify-ca` | Verify against CA | Yes |
| `verify-full` | Verify CA + hostname | Yes |

`client_cert_path` and `client_key_path` must always appear together.

## Verify-service: listener

| Field | Purpose |
|-------|---------|
| `listener.tls.cert_path` | Server certificate |
| `listener.tls.key_path` | Server private key |
| `listener.tls.client_ca_path` | CA for mTLS (optional) |

### HTTP

```yaml
listener:
  bind_addr: 0.0.0.0:8080
```

Omit the `tls` block entirely.

### HTTPS

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

### HTTPS with mTLS

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

When `tls` is present, `cert_path` and `key_path` are both required. `client_ca_path` is always optional.

## Verify-service: database connections

Both `verify.source` and `verify.destination` use the same shape: a URL with `sslmode` query parameter, plus an optional nested `tls` block for certificate file paths.

### Source with `verify-full` and client certificates

```yaml
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
```

### Destination with `verify-ca` (CA only)

```yaml
verify:
  destination:
    url: postgresql://verify_target:secret@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
```

### Passwordless client-certificate auth

Omit the password from the URL and supply both `client_cert_path` and `client_key_path`:

```yaml
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
```

### Rules

| Rule | Applies to |
|------|-----------|
| `sslmode=verify-ca` or `sslmode=verify-full` requires `tls.ca_cert_path` | Source, destination |
| `client_cert_path` and `client_key_path` must appear together | Source, destination |
| URL scheme must be `postgresql://` or `postgres://` | Source, destination |

## Quick reference: TLS field mapping

| Component | Config path | Fields |
|-----------|-------------|--------|
| Runner webhook listener | `webhook.mode`, `webhook.tls.*` | `cert_path`, `key_path`, `client_ca_path` (optional) |
| Runner destination (URL) | `mappings[].destination.url` | `sslmode`, `sslrootcert`, `sslcert`, `sslkey` in query params |
| Runner destination (decomposed) | `mappings[].destination.tls.*` | `mode`, `ca_cert_path`, `client_cert_path`, `client_key_path` |
| Verify listener | `listener.tls.*` | `cert_path`, `key_path`, `client_ca_path` (optional) |
| Verify source/destination | `verify.source.tls.*`, `verify.destination.tls.*` | `ca_cert_path`, `client_cert_path`, `client_key_path` |


# docs/operator-guide/setup-sql.md

# Source & Destination Setup

Before starting the runner, you must prepare both databases: CockroachDB needs changefeeds configured, and PostgreSQL needs the correct permissions granted. The runner only receives webhook payloads — it does not create changefeeds, databases, schemas, or destination tables for you.

**Prerequisite:** TLS certificates must already exist before writing runner configs, since configuration references certificate paths. See [TLS Configuration](tls-configuration.md).

## Part 1: CockroachDB source

### Step 1: Enable rangefeeds

Run once per CockroachDB cluster:

```sql
SET CLUSTER SETTING kv.rangefeed.enabled = true;
```

This persists across restarts.

### Step 2: Capture a cursor per source database

For each source database, capture the current logical timestamp immediately before creating changefeeds:

```sql
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;
```

The result is a decimal like `1745877420457561000.0000000000`. Paste this value into every `CREATE CHANGEFEED` for that database. One cursor per database keeps all changefeeds aligned on the same start boundary.

### Step 3: Create one changefeed per mapping

For each mapping in your runner config:

```sql
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders
INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D'
WITH cursor = '1745877420457561000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';
```

| Option | Value | Purpose |
|--------|-------|---------|
| `cursor` | Decimal from step 2 | Consistent start point |
| `initial_scan` | `'yes'` | Snapshots existing data before streaming |
| `envelope` | `'enriched'` | Payload format the runner expects |
| `resolved` | Interval, e.g. `'5s'` | Watermark emission frequency |

#### Sink URL format

```
webhook-<base_url>/ingest/<mapping_id>?ca_cert=<percent-encoded-base64-cert>
```

- `<base_url>` — The externally reachable runner URL, e.g. `https://runner.example.internal:8443`
- `<mapping_id>` — Must exactly match the `id` field in the runner config (case-sensitive)
- `<percent-encoded-base64-cert>` — PEM CA certificate base64-encoded with no line breaks, then percent-encoded

Encode the CA certificate:

```bash
CA_CERT_B64=$(cat /config/certs/ca.crt | base64 -w0 | python3 -c 'import urllib.parse,sys; print(urllib.parse.quote(sys.stdin.read().strip()))')
```

#### HTTP sinks (development only)

When the runner uses `mode: http`, omit `ca_cert` and use `webhook-http://`:

```sql
CREATE CHANGEFEED FOR TABLE demo_a.public.customers
INTO 'webhook-http://runner.example.internal:8080/ingest/app-a'
WITH cursor = '1745877420457561000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';
```

### Worked example: two databases, two mappings

```sql
-- Connect to: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require

-- Step 1: Enable rangefeeds
SET CLUSTER SETTING kv.rangefeed.enabled = true;

-- Step 2: Database demo_a
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;
-- Result: 1745877420457561000.0000000000

-- Step 3: Mapping app-a
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders
INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D'
WITH cursor = '1745877420457561000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';

-- Step 2: Database demo_b
USE demo_b;
SELECT cluster_logical_timestamp() AS changefeed_cursor;
-- Result: 1745877420459999000.0000000000

-- Step 3: Mapping app-b
CREATE CHANGEFEED FOR TABLE demo_b.public.invoices, demo_b.public.invoice_lines
INTO 'webhook-https://runner.example.internal:8443/ingest/app-b?ca_cert=LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D'
WITH cursor = '1745877420459999000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';
```

### CockroachDB checklist

- [ ] `kv.rangefeed.enabled = true` is set on the source cluster
- [ ] One cursor captured per source database before creating changefeeds
- [ ] Every `CREATE CHANGEFEED` uses `cursor`, `initial_scan = 'yes'`, `envelope = 'enriched'`, and `resolved`
- [ ] Each sink URL ends with `/ingest/<mapping_id>` matching the runner config
- [ ] Table names are fully qualified (`database.schema.table`)
- [ ] The `ca_cert` query parameter is correctly percent-encoded
- [ ] The runner HTTPS endpoint is reachable from the source cluster

## Part 2: PostgreSQL destination

The runtime role needs specific permissions on each destination database. The runner creates the `_cockroach_migration_tool` helper schema and its tracking tables automatically — you only need to grant access.

### Step 1: Database-level grants

Run once per destination database and runtime role:

```sql
GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;
```

- `CONNECT` — Allows the role to log into the database
- `CREATE` — Allows the role to create `_cockroach_migration_tool`

### Step 2: Schema-level grants

Run once per mapped schema and runtime role:

```sql
GRANT USAGE ON SCHEMA public TO migration_user_a;
```

### Step 3: Table-level grants

Run once per mapped table and runtime role:

```sql
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.orders TO migration_user_a;
```

| Privilege | Why the runner needs it |
|-----------|------------------------|
| `SELECT` | Check existing rows during reconciliation |
| `INSERT` | Write new rows from changefeed events |
| `UPDATE` | Update rows when changefeeds carry modifications |
| `DELETE` | Delete rows when changefeeds carry deletion events |

### What the runner creates

After grants are in place, the runner automatically creates:

- Schema: `_cockroach_migration_tool`
- Tables: `stream_state`, `table_sync_state`, and per-mapping helper tables

These are owned by the runtime role — no additional grants needed.

### Worked example: two databases, two mappings

```sql
-- Destination database app_a
GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;
GRANT USAGE ON SCHEMA public TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.orders TO migration_user_a;

-- Destination database app_b
GRANT CONNECT, CREATE ON DATABASE app_b TO migration_user_b;
GRANT USAGE ON SCHEMA public TO migration_user_b;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.invoices TO migration_user_b;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.invoice_lines TO migration_user_b;
```

### PostgreSQL checklist

- [ ] Every destination database has `GRANT CONNECT, CREATE ON DATABASE <database> TO <role>`
- [ ] Every mapped schema has `GRANT USAGE ON SCHEMA <schema> TO <role>`
- [ ] Every mapped table has `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE <schema>.<table> TO <role>`
- [ ] The runtime role exists and can authenticate
- [ ] Destination databases, schemas, and tables all exist before the runner starts
- [ ] When adding tables to a mapping, grant privileges before restarting the runner

## SQL generator scripts

The repository includes scripts that generate the SQL above from a small YAML config. They produce auditable SQL files for manual execution.

### CockroachDB source SQL generator

`scripts/generate-cockroach-setup-sql.sh` reads a YAML config and renders `SET CLUSTER SETTING`, cursor capture, and `CREATE CHANGEFEED` statements. Output files: `cockroach-<database>-setup.sql` and `cockroach-all-setup.sql`.

Required config shape:

```yaml
cockroach:
  url: "postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require"
webhook:
  base_url: "https://runner.example.internal:8443"
  ca_cert_path: "ca.crt"
  resolved: "5s"
mappings:
  - id: "app-a"
    source:
      database: "demo_a"
      tables:
        - "public.customers"
        - "public.orders"
```

Usage:

```bash
./scripts/generate-cockroach-setup-sql.sh [--dry-run] [--output-dir ./output] ./cockroach-setup-config.yml
```

### PostgreSQL grants SQL generator

`scripts/generate-postgres-grants-sql.sh` reads a YAML config and renders per-database grant statements. Output files: `postgres-<database>-grants.sql` and `postgres-all-grants.sql`.

Required config shape:

```yaml
mappings:
  - id: "app-a"
    destination:
      database: "app_a"
      runtime_role: "migration_user_a"
      tables:
        - "public.customers"
        - "public.orders"
```

Usage:

```bash
./scripts/generate-postgres-grants-sql.sh [--dry-run] [--output-dir ./output] ./postgres-grants-config.yml
```

### Dependencies

Both scripts require `bash`, `envsubst`, and `sort`. The Cockroach script additionally requires `base64`. YAML parsing uses `yq` (preferred) or falls back to `python3`.

```bash
nix develop   # provides all dependencies
```

### Common flags

| Flag | Effect |
|------|--------|
| `--help` | Print usage |
| `--dry-run` | Print files that would be generated without writing |
| `--output-dir <path>` | Write output to a directory other than `./output` |

Both scripts fail fast with an `error:` message on missing files, absent keys, missing dependencies, or invalid table names.

## Order of operations

1. Enable rangefeeds on the CockroachDB cluster
2. Grant PostgreSQL destination permissions
3. Capture changefeed cursors
4. Create changefeeds pointing at `/ingest/{mapping_id}`
5. Start the runner

CockroachDB retries webhook deliveries, so changefeeds can be created before the runner is listening — data simply won't flow until the runner is reachable.


# docs/operator-guide/verify-service.md

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
| `source` | object | yes | Source (CockroachDB or PostgreSQL) database connection |
| `destination` | object | yes | Destination PostgreSQL database connection |
| `raw_table_output` | boolean | no | Enable `POST /tables/raw` endpoint. Defaults to `false`. |

#### `verify.source` and `verify.destination`

Both use the same shape:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `url` | string | yes | Connection URL. Must use `postgresql://` or `postgres://` scheme. Include `sslmode` as a query parameter. |
| `tls` | object | no | File paths for TLS certificates and keys |

```yaml
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    url: postgresql://verify_target:secret@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
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

When `sslmode=verify-ca` or `sslmode=verify-full`, `ca_cert_path` is required. `client_cert_path` and `client_key_path` must always appear as a pair. For passwordless client-certificate auth, omit the password from the URL.

### Full example

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
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    url: postgresql://verify_target@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
      client_cert_path: /config/certs/destination-client.crt
      client_key_path: /config/certs/destination-client.key
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

Request body (all fields optional POSIX regular expressions):

```json
{
  "include_schema": "^public$",
  "include_table": "^(accounts|orders)$"
}
```

| Field | Description |
|-------|-------------|
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
{"error": {"category": "request_validation", "code": "unknown_field", "message": "request body contains an unsupported field", "details": [{"field": "extra", "reason": "unknown field"}]}}
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

{"database": "source", "schema": "public", "table": "accounts"}
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
    url: postgresql://verify_source:source-password@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
  destination:
    url: postgresql://verify_target:target-password@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
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
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
  destination:
    url: postgresql://verify_target:secret@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
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


# docs/operator-guide/architecture.md

# Architecture

How the migration pipeline works internally — sufficient for understanding operator behavior, diagnosing issues, and reasoning about data flow.

## System overview

```
                 ┌─ CockroachDB source ─┐
                 │  changefeeds push     │
                 │  row batches via      │
                 │  webhook-https://     │
                 └──────────┬────────────┘
                            │
                            ▼
  ┌──────────────────────────────────────────────────────┐
  │                    runner container                   │
  │                                                      │
  │  ┌────────────────┐    ┌──────────────────────────┐  │
  │  │ webhook listener│    │     reconcile loop       │  │
  │  │ POST /ingest/:id│───▶│  upsert → delete passes  │  │
  │  └────────────────┘    └──────────┬───────────────┘  │
  │                                   │                   │
  └───────────────────────────────────┼───────────────────┘
                                      │
                                      ▼
  ┌──────────────────────────────────────────────────────┐
  │                PostgreSQL destination                 │
  │                                                      │
  │  ┌─────────────────────────┐  ┌───────────────────┐  │
  │  │ _cockroach_migration_tool│  │  real tables      │  │
  │  │ (helper shadow tables)  │  │  (constrained)    │  │
  │  └─────────────────────────┘  └───────────────────┘  │
  └──────────────────────────────────────────────────────┘
                ▲                       ▲
                │                       │
  ┌─────────────┴───────────────────────┴────────────────┐
  │                verify-service container               │
  │   reads both databases, compares row-by-row           │
  └──────────────────────────────────────────────────────┘
```

## Runner internals

### Webhook listener

The runner exposes `POST /ingest/{mapping_id}` on `webhook.bind_addr`. CockroachDB changefeeds push JSON batches to this endpoint. Each batch contains row events (`c` create, `u` update, `d` delete, `r` refresh) and periodically a `resolved` watermark.

On receiving a batch the runner:

1. Validates the payload structure and `mapping_id`.
2. Opens a PostgreSQL transaction.
3. Applies each row mutation to the corresponding helper shadow table inside `_cockroach_migration_tool`.
4. Updates `latest_received_resolved_watermark` in `stream_state` for resolved messages.
5. Commits and returns `200 OK`.

The runner **never** touches the real constrained tables during webhook handling. This keeps the hot path fast — shadow tables have no foreign keys, secondary indexes, or serving constraints.

### Reconciliation loop

The reconcile loop runs independently on a timer set by `reconcile.interval_secs`. Each tick, for every mapping:

1. **Upsert pass** (parents before children, respecting foreign key order):
   - `INSERT INTO real_table (...) SELECT ... FROM shadow_table ON CONFLICT (...) DO UPDATE ...`
2. **Delete pass** (children before parents, reverse order):
   - `DELETE FROM real_table WHERE NOT EXISTS (SELECT 1 FROM shadow_table WHERE pk matches)`
3. If all tables succeed, advances `latest_reconciled_resolved_watermark` in `stream_state` and updates `last_successful_sync_watermark` per table in `table_sync_state`.

After a successful reconcile pass the real tables and shadow tables are identical for all rows that existed at the reconciled watermark.

### What `reconcile.interval_secs` does operationally

`reconcile.interval_secs` is the number of seconds between two reconciliation passes. Setting it higher gives the destination database more breathing room between bulk upserts and deletes. Setting it lower reduces the lag between webhook ingestion and real-table convergence.

Operationally:

- During bulk initial scans (when the changefeed snapshots millions of rows), longer intervals reduce destination load.
- During steady-state catch-up, shorter intervals keep real tables closer to live.
- The reconcile loop skips a pass if the previous pass is still running — it does not stack concurrent passes.

A value of 30 seconds is a reasonable default for most workloads.

## `_cockroach_migration_tool` helper schema

The runner creates one `_cockroach_migration_tool` schema per destination database. It holds two kinds of objects:

### Tracking tables

| Table | Purpose | Key columns |
|-------|---------|-------------|
| `stream_state` | Per-mapping stream lifecycle | `mapping_id`, `source_database`, `source_job_id`, `starting_cursor`, `latest_received_resolved_watermark`, `latest_reconciled_resolved_watermark`, `stream_status` |
| `table_sync_state` | Per-table reconciliation status | `mapping_id`, `source_table_name`, `helper_table_name`, `last_successful_sync_time`, `last_successful_sync_watermark`, `last_error` |

### Diagnostic queries

Check stream progress (how far CDC has delivered vs how far reconciliation has caught up):

```sql
SELECT mapping_id,
       latest_received_resolved_watermark AS received_up_to,
       latest_reconciled_resolved_watermark AS reconciled_up_to,
       stream_status
FROM _cockroach_migration_tool.stream_state;
```

Check per-table reconciliation status and errors:

```sql
SELECT mapping_id,
       source_table_name,
       last_successful_sync_time,
       last_successful_sync_watermark,
       last_error
FROM _cockroach_migration_tool.table_sync_state;
```

Count shadow rows waiting to be merged into real tables:

```sql
SELECT schemaname, tablename, n_live_tup
FROM pg_stat_user_tables
WHERE schemaname = '_cockroach_migration_tool';
```

### Helper shadow tables

For each mapped source table (e.g. `public.customers`), the runner creates a corresponding shadow table named `{mapping_id}__{schema}__{table}` (e.g. `app-a__public__customers`). Shadow tables mirror the real table's data columns but with:

- **No foreign keys** — avoids constraint ordering problems during upserts
- **No secondary indexes** — keeps writes fast
- **A matching primary key index** — enables efficient upsert and anti-join delete passes

Shadow tables are the durable landing zone for changefeed batches. If the runner crashes between webhook ingestion and reconcile, no data is lost — the shadow table holds every received row and reconciliation resumes from where it left off.

## Verify-service internals

### How table comparison works

When a verify job starts (`POST /jobs`), the verify-service:

1. **Connects to both databases** using the configured `verify.source` and `verify.destination` connection strings.
2. **Discovers all user tables** on each side by querying `pg_class` / `pg_namespace`, excluding system schemas (`pg_catalog`, `information_schema`, `crdb_internal`, `pg_extension`).
3. **Applies filters** from the job request body:
   - `include_schema` / `include_table` — POSIX regexes that tables must match to be verified (default `.*`, matching everything)
   - `exclude_schema` / `exclude_table` — POSIX regexes that exclude matching tables
4. **Compares table lists** across the two databases:
   - Tables in source but not destination → reported as **missing**
   - Tables in destination but not source → reported as **extraneous**
   - Tables in both → **verified** (columns compared, then row data)
5. **Compares column definitions** for each verified table, reporting mismatches.
6. **Splits each table into shards** by primary key range (default 8 shards per table).
7. **Compares row data** within each shard in parallel (default 8 concurrent workers), batching 1000 rows at a time from both sides.
8. **Reports findings** — matching rows, missing rows, extraneous rows, and per-column value mismatches.

### Runner ↔ verify-service separation

The runner and verify-service are completely independent processes that share no runtime state:

- The **runner** writes webhook data into PostgreSQL and reconciles it into real tables. It does not verify correctness.
- The **verify-service** reads both databases and compares them. It does not participate in data movement.

They communicate only through the database: the runner populates destination tables; the verify-service queries them. This separation means you can run verification at any time, with any cadence, without affecting the migration pipeline.

The verify-service compares **source directly against destination real tables** — not against the `_cockroach_migration_tool` shadow tables. So a verify job measures the end-to-end correctness of the full pipeline: changefeed → webhook → shadow table → reconcile → real table.

### Why verify-service uses separate database connections

The verify-service connects to both source and destination using the `postgresql://` URLs in its config. It can connect to any PostgreSQL-compatible database — CockroachDB, standard PostgreSQL, or managed services. It does not depend on the runner's configuration or connection state. This also means the verify-service can compare two databases that are not connected to any runner at all — useful for one-off comparisons outside a migration.

## Failure modes

### Webhook ingestion failures

When the runner cannot process an incoming changefeed batch:

1. **The transaction is rolled back.** No partial data lands in shadow tables. `500 Internal Server Error` is returned to CockroachDB.
2. **CockroachDB retries.** Changefeeds retry failed deliveries with backoff. During retries the stream advances — the changefeed buffers events and will deliver the current state (not a replay of historical events) on reconnect.
3. **No data is lost.** CockroachDB guarantees at-least-once delivery. The shadow tables remain in their last-committed state. When delivery resumes, newer events arrive and are processed normally. The reconcile loop catches up the real tables from whatever is in the shadow tables.

Common causes: destination database unavailable (transient network blip or PostgreSQL restart), constraint violations from malformed payloads, or shadow table DDL failures during schema bootstrapping.

**What to inspect:**
- Runner stderr logs — every webhook error includes the mapping ID and a description.
- `cockroach_migration_tool_runner_` Prometheus metrics at `GET /metrics`.
- CockroachDB changefeed job status: `SHOW CHANGEFEED JOB <job_id>` on the source cluster.

### Reconciliation failures

When a reconcile pass fails for one or more tables within a mapping:

1. **The failed table reports an error** in `_cockroach_migration_tool.table_sync_state.last_error`. This field stores the most recent error message for that table.
2. **`latest_reconciled_resolved_watermark` is not advanced** for the mapping — the reconcile loop only advances the watermark when every table in the mapping succeeds.
3. **The next reconcile pass retries** on the next timer tick. The reconcile loop does not back off; it retries at the configured interval.
4. **Real tables are not modified** for the failed pass. All changes remain staged in shadow tables. Successful tables from the same mapping are also not advanced because the watermark tracks the whole mapping.
5. **The reconcile loop does not block on a running pass.** If a previous reconcile is still in flight when the timer fires, the tick is skipped.

Common causes: foreign key violations (destination data that conflicts with the shadow-table state), missing required destination columns, or destination database becoming unavailable mid-pass.

**What to inspect:**
- `_cockroach_migration_tool.table_sync_state` — per-table `last_error` and `last_successful_sync_time`:
  ```sql
  SELECT mapping_id, source_table_name,
         last_successful_sync_time,
         last_error
  FROM _cockroach_migration_tool.table_sync_state
  WHERE last_error IS NOT NULL;
  ```
- `_cockroach_migration_tool.stream_state` — check if `latest_reconciled_resolved_watermark` has stalled relative to `latest_received_resolved_watermark`:
  ```sql
  SELECT mapping_id,
         latest_received_resolved_watermark AS received_up_to,
         latest_reconciled_resolved_watermark AS reconciled_up_to,
         stream_status
  FROM _cockroach_migration_tool.stream_state;
  ```
- Runner stderr logs — reconciliation errors are logged with the mapping ID and the specific table that failed.
- Shadow table row counts versus real table row counts — if reconciled watermark is stalled but shadow tables contain rows, reconciliation is blocked on something (constraint, missing column, etc.).

### Runner process crash

- **Shadow tables are durable.** All committed webhook batches survive a crash.
- **Reconciliation resumes from the last committed watermark.** On restart the runner bootstraps `_cockroach_migration_tool` (no-op if already present), then begins the reconcile loop.
- **Stream state is in-database.** `stream_status`, watermarks, and per-table sync state persist across restarts.
- **No double processing.** The runner tracks watermarks in the database; it does not reprocess already-reconciled batches.

### Verify-service failures

- **Connection failures** during a verify job produce `source_access` or `destination_access` errors. The job status becomes `failed` with the error category and message in the response body.
- **A running job survives transient connection errors** to one database — the other database's shards continue in parallel. The job fails only when all workers have exhausted their shards or hit unrecoverable errors.
- **Job state is in-memory.** If the verify-service process crashes, all job history is lost. Start a new job after restart.
- **Only one job runs at a time.** `POST /jobs` returns `409 Conflict` if a job is already running. Stop it first with `POST /jobs/{job_id}/stop`.


# docs/operator-guide/runner.md

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


# docs/operator-guide/troubleshooting.md

# Troubleshooting

Common failures and diagnostic steps. For understanding the internal state of the migration pipeline, see the diagnostic queries in [Architecture — `_cockroach_migration_tool`](architecture.md#_cockroach_migration_tool-helper-schema).

## Runner

### `validate-config` exits nonzero

**Checks:**
- All three top-level keys (`webhook`, `reconcile`, `mappings`) are present.
- `webhook.bind_addr` is a valid `host:port` string.
- When `webhook.mode` is `https`, `webhook.tls` with `cert_path` and `key_path` is present.
- When `webhook.mode` is `http`, there is no `webhook.tls` block.
- `reconcile.interval_secs` is a positive integer.
- `mappings` contains at least one entry with a unique `id`.
- Each `mappings[].source.tables` entry is schema-qualified (`public.customers`, not `customers`).
- `mappings[].destination` uses either a `url` string or decomposed fields (`host`, `port`, `database`, `user`, `password`) — never both.
- When using decomposed `tls` with `mode: verify-ca` or `mode: verify-full`, `ca_cert_path` is set.
- `client_cert_path` and `client_key_path` always appear together.

### `validate-config --deep` cannot reach destination

**Checks:**
- The Docker container can reach the destination host and port. Use `--network host` if the database is on the host network.
- The destination PostgreSQL accepts connections from the runner container's IP.
- `sslmode` is correct. If the database requires TLS, use `verify-ca` or `verify-full` and mount the CA certificate.
- If using client certificates, both `client_cert_path` and `client_key_path` are mounted correctly.

### Changefeeds get connection refused

**Checks:**
- The runner is listening on an address reachable from the CockroachDB cluster. `0.0.0.0` is fine; `127.0.0.1` is only local.
- The CockroachDB cluster can resolve the runner hostname and reach the port.
- The `ca_cert` in the changefeed sink URL matches the CA that signed the runner's server certificate, properly percent-encoded.
- The sink URL uses `webhook-https://` (not `webhook-http://`) when the runner uses HTTPS mode.
- The runner container port is mapped correctly: `-p 8443:8443`.

### `POST /ingest/{mapping_id}` returns 404

**Checks:**
- The `mapping_id` in the changefeed sink URL exactly matches an `id` in the runner config (case-sensitive).
- The runner was restarted after the mapping was added.

### `POST /ingest/{mapping_id}` returns 400

**Checks:**
- The `length` field equals the number of entries in `payload`.
- All required fields are present in each payload entry: `key`, `op`, `source` (with `database_name`, `schema_name`, `table_name`), and `after` for `c`, `u`, `r` operations.

## Verify-Service

> The verify-service does **not** expose `/healthz`. Use `GET /metrics` or a TCP port check to confirm the service is alive. See [Verify-Service — Health checking the verify-service](verify-service.md#health-checking-the-verify-service).

### `verify-service validate-config` exits nonzero

**Checks:**
- Both `listener` and `verify` keys are present.
- `listener.bind_addr` is a valid `host:port` string.
- If `listener.tls` is present, both `cert_path` and `key_path` are set.
- `verify.source.url` and `verify.destination.url` use `postgres://` or `postgresql://`.
- When `sslmode=verify-ca` or `sslmode=verify-full`, the corresponding `tls.ca_cert_path` is set.
- `client_cert_path` and `client_key_path` always appear as a pair.

### `POST /jobs` returns 409

A verify job is already running. Only one job at a time.

**Fix:** Poll `GET /jobs/{job_id}` until it finishes, or stop it with `POST /jobs/{job_id}/stop`.

### `GET /jobs/{job_id}` returns 404

The verify-service process restarted since the job was created. Job state is in-memory.

**Fix:** Start a new job with `POST /jobs`.

### Job fails with `source_access` error

**Checks:**
- The verify-service container can reach the source database host and port.
- The URL in `verify.source.url` is correct, including `sslmode`.
- All required TLS certificate files are mounted and paths in `verify.source.tls` match.
- The source PostgreSQL user has read permission on the tables being verified.

### Job fails with `destination_access` error

Same checks as `source_access`, applied to `verify.destination`.

### Job reports mismatches

1. Check `result.mismatch_summary.affected_tables` for affected tables.
2. Check `result.findings` for per-row detail: `mismatching_columns`, `source_values`, `destination_values`.
3. Decide whether to re-run verification after fixing data, or accept the mismatches.

## General

### Container cannot access mounted certificates ("file not found")

**Fix:**
- Verify the volume mount: `-v "$(pwd)/config:/config:ro"` maps local `./config` to `/config` inside the container.
- Verify file permissions — the container process needs read access to all certs and keys.
- Config paths must reference the container mount target (`/config/certs/server.crt`), not the host path.

### Port already in use

**Fix:**
- Change `webhook.bind_addr` or `listener.bind_addr` to a different port.
- Or map a different host port: `-p 9443:8443` instead of `-p 8443:8443`.

### Stale image

**Fix:**
- Force a fresh pull: `docker pull ghcr.io/<owner>/runner-image:<git-sha>`.
- Verify the image digest matches the published build.


