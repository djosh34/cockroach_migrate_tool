You are `opencode-go/kimi-k2.6`.

You are a style reviewer only. You may read only the documentation bundle attached to this prompt. Do not read repository code, tests, workflows, scripts, or any project files outside that docs bundle.

Your task:

1. Review the docs only for wording, readability, scanability, tone, and style.
2. Do not suggest factual changes unless a sentence is so unclear that it creates a language problem.
3. Prefer a small number of high-value editorial suggestions over a long nit list.
4. Do not rewrite the docs directly.

Output format:

- `Overall impression:` one short paragraph
- `High-value style suggestions:` flat bullet list
- `Suggestions to ignore:` flat bullet list

Keep the response concise.


Documentation bundle follows.

=== FILE: docs/public_image_operator_guide/README.md ===
# Operator Guide — Published Images

This guide covers deploying the CockroachDB-to-PostgreSQL migration tool from published container images — from pulling images to verifying data integrity after migration.

## Images

| Image | Purpose | Primary registry |
|-------|---------|-------------------|
| `runner-image` | Receives CockroachDB changefeed webhooks and writes rows into PostgreSQL destinations | GHCR |
| `verify-image` | Compares source and destination data to confirm migration correctness | GHCR |

See **[Image References](image-references.md)** for exact pull commands and tag conventions.

## Operator workflow

```
1.  Pull images                →  image-references.md
2.  Prepare source CockroachDB →  source-setup.md → source-setup/cockroachdb-setup.md
3.  Grant destination perms    →  destination-grants.md → destination-setup/postgresql-grants.md
4.  Configure and run runner   →  runner.md → runner/getting-started.md
5.  Configure and run verify   →  verify-service.md → verify/getting-started.md
6.  Run a verify job           →  verify/job-lifecycle.md
7.  Troubleshoot problems      →  troubleshooting.md
```

## All pages

| Document | What it covers |
|----------|---------------|
| [Image References](image-references.md) | GHCR pull commands, tag format, Quay mirror |
| [Source Setup](source-setup.md) | CockroachDB setup overview |
| [CockroachDB setup](source-setup/cockroachdb-setup.md) | Full SQL walkthrough, sink URL encoding, checklist |
| [Destination Grants](destination-grants.md) | PostgreSQL grants overview |
| [PostgreSQL grants](destination-setup/postgresql-grants.md) | Per-database SQL, worked examples, checklist |
| [Runner](runner.md) | Runner overview, sub-page index |
| [Runner getting started](runner/getting-started.md) | Pull, configure, validate, and run |
| [Runner configuration](runner/configuration.md) | Full YAML reference |
| [Runner endpoints](runner/endpoints.md) | `/healthz`, `/metrics`, `/ingest/{mapping_id}` |
| [Verify Service](verify-service.md) | Verify-service overview, sub-page index |
| [Verify getting started](verify/getting-started.md) | Pull, configure, validate, and run |
| [Verify configuration](verify/configuration.md) | Full YAML reference |
| [Verify job lifecycle](verify/job-lifecycle.md) | Start, poll, and stop verify jobs |
| [TLS reference](tls-reference.md) | TLS configuration for all components |
| [Troubleshooting](troubleshooting.md) | Common setup failures and diagnostics |

=== END FILE ===

=== FILE: docs/public_image_operator_guide/destination-grants.md ===
# Destination PostgreSQL Grants

The runtime role needs specific permissions on each destination PostgreSQL database. The runner creates its own `_cockroach_migration_tool` helper schema after these grants are in place.

## Three steps

1. **Database-level** — `GRANT CONNECT, CREATE` so the role can connect and create the helper schema.
2. **Schema-level** — `GRANT USAGE` on each mapped schema.
3. **Table-level** — `GRANT SELECT, INSERT, UPDATE, DELETE` on each mapped table.

## Detailed guide

The full walkthrough with per-database examples, multi-mapping examples, and a pre-flight checklist:

**[PostgreSQL destination grants](./destination-setup/postgresql-grants.md)**

## Quick reference

```sql
-- Per destination database (replace <database> and <runtime_role>)
GRANT CONNECT, CREATE ON DATABASE <database> TO <runtime_role>;

-- Per mapped schema
GRANT USAGE ON SCHEMA <schema> TO <runtime_role>;

-- Per mapped table
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE <schema>.<table> TO <runtime_role>;
```

> **Important:** The runner does not create destination databases, schemas, or tables. Those must exist before the runner starts.

See the [detailed guide](./destination-setup/postgresql-grants.md) for worked examples and the complete checklist.

=== END FILE ===

=== FILE: docs/public_image_operator_guide/destination-setup/postgresql-grants.md ===
# PostgreSQL Destination Grants

You must grant the runtime role access to each destination PostgreSQL database before starting the runner. The runner creates its own helper schema and tracking tables after these grants are in place.

## What you need

- PostgreSQL SQL access as a database owner, schema owner, table owner, or superuser.
- The destination database name for each mapping.
- The runtime login role that the runner uses to connect.
- Every mapped destination schema and table.

> **The runner does not create the destination database, schemas, or tables.** It only creates the `_cockroach_migration_tool` helper schema and its internal tracking tables.

## Step 1: Grant database access

Run once per destination database and runtime role:

```sql
GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;
```

- `CONNECT` lets the runtime log into the database.
- `CREATE` lets the runtime create `_cockroach_migration_tool` inside that database.

## Step 2: Grant schema usage

Run once per mapped destination schema and runtime role:

```sql
GRANT USAGE ON SCHEMA public TO migration_user_a;
```

## Step 3: Grant table DML privileges

Run once per mapped destination table and runtime role:

```sql
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.orders TO migration_user_a;
```

| Privilege | Why the runner needs it |
| --------- | ----------------------- |
| `SELECT` | Check existing rows during reconciliation |
| `INSERT` | Write new rows from changefeed events |
| `UPDATE` | Update existing rows when changefeed events carry modifications |
| `DELETE` | Delete rows when changefeed events carry deletion payloads |

## Worked example: two databases, two mappings

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

## What the runner creates

After the grants are in place, the runtime creates these objects automatically:

- Schema: `_cockroach_migration_tool`
- Table: `_cockroach_migration_tool.stream_state`
- Table: `_cockroach_migration_tool.table_sync_state`
- Additional helper tables per mapping and mapped source table

You do not need to grant privileges on `_cockroach_migration_tool` ahead of time because the runtime role owns the objects it creates there.

## Checklist

- [ ] Every destination database has `GRANT CONNECT, CREATE ON DATABASE <database> TO <runtime_role>`.
- [ ] Every mapped schema has `GRANT USAGE ON SCHEMA <schema> TO <runtime_role>`.
- [ ] Every mapped table has `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE <schema>.<table> TO <runtime_role>`.
- [ ] The runtime role exists and can authenticate against the destination PostgreSQL server.
- [ ] The destination databases, schemas, and tables all exist before the runner starts.

> **Adding new tables?** If you add new tables to a mapping, you must grant privileges on those tables before restarting the runner. The runner does not attempt to grant privileges itself.

## See also

- [Runner getting started](../runner/getting-started.md) — write runner config and start the runner
- [TLS reference](../tls-reference.md) — runner destination TLS configuration
- [Troubleshooting](../troubleshooting.md) — common destination connectivity errors

=== END FILE ===

=== FILE: docs/public_image_operator_guide/image-references.md ===
# Image References

## Primary registry — GHCR

Images are published to the **GitHub Container Registry (GHCR)** on every push to the repository. The tag is the full Git commit SHA.

### Pull commands

```bash
# Runner
docker pull ghcr.io/djosh34/runner-image:<git-sha>

# Verify-service
docker pull ghcr.io/djosh34/verify-image:<git-sha>
```

Replace `<git-sha>` with the full 40-character commit SHA of the version you want to deploy. Obtain a valid published SHA from the GHCR package pages:

- Runner: `https://github.com/djosh34/cockroach_migrate_tool/pkgs/container/runner-image` — click **View all tagged versions** to see available SHAs.
- Verify: `https://github.com/djosh34/cockroach_migrate_tool/pkgs/container/verify-image` — click **View all tagged versions** to see available SHAs.

Both images are multi-platform manifests supporting `linux/amd64` and `linux/arm64`.

## Quay mirror

Images are also mirrored to Quay, but **GHCR is the source of truth**. Quay mirrors lag behind GHCR and should not be used to determine availability.

```
quay.io/<quay-organization>/<runner-repository>:<git-sha>
quay.io/<quay-organization>/<verify-repository>:<git-sha>
```

The repository names are determined by the CI variables `RUNNER_IMAGE_REPOSITORY` and `VERIFY_IMAGE_REPOSITORY`. The paths shown in GHCR (`runner-image`, `verify-image`) are examples, not guaranteed Quay paths. Consult your organization's CI configuration for the exact repository names.

## Authenticating to GHCR

```bash
echo "$GITHUB_TOKEN" | docker login ghcr.io -u "$GITHUB_USERNAME" --password-stdin
```

The token needs the `read:packages` scope.

## Running a container

Both images default to running their respective subcommands. Pass arguments after the image name:

```bash
# Runner: validate config (offline)
docker run --rm \
  -v ./config:/config:ro \
  ghcr.io/djosh34/runner-image:<git-sha> \
  validate-config --config /config/runner.yml

# Runner: validate config (deep — checks destination connectivity)
docker run --rm \
  -v ./config:/config:ro \
  --network host \
  ghcr.io/djosh34/runner-image:<git-sha> \
  validate-config --config /config/runner.yml --deep

# Runner: start the service
docker run --rm \
  -v ./config:/config:ro \
  -p 8443:8443 \
  ghcr.io/djosh34/runner-image:<git-sha> \
  run --config /config/runner.yml

# Verify-service: validate config
docker run --rm \
  -v ./config:/config:ro \
  ghcr.io/djosh34/verify-image:<git-sha> \
  verify-service validate-config --config /config/verify-service.yml

# Verify-service: start the service
docker run --rm \
  -v ./config:/config:ro \
  -p 8080:8080 \
  ghcr.io/djosh34/verify-image:<git-sha> \
  verify-service run --config /config/verify-service.yml
```

> **Note:** The verify-image entrypoint is `molt` and its default command is `verify-service`. Always include the `verify-service` subcommand explicitly — especially when overriding `command` in Docker Compose.

## Log format

Both images support structured JSON logging via `--log-format`. The flag is placed differently depending on the image:

| Value | Behavior |
| ----- | -------- |
| `text` | Human-readable console output (default) |
| `json` | Structured JSON for log aggregators |

- **Runner:** `--log-format` is a global flag that precedes the subcommand:

```bash
docker run --rm \
  ghcr.io/djosh34/runner-image:<git-sha> \
  --log-format json \
  validate-config --config /config/runner.yml
```

- **Verify-service:** `--log-format` is a flag on the `validate-config` and `run` subcommands:

```bash
docker run --rm \
  ghcr.io/djosh34/verify-image:<git-sha> \
  verify-service validate-config --log-format json --config /config/verify-service.yml
```

## See also

- [Runner getting started](runner/getting-started.md) — full runner setup walkthrough
- [Verify getting started](verify/getting-started.md) — full verify-service setup walkthrough

=== END FILE ===

=== FILE: docs/public_image_operator_guide/index.md ===
# Operator Guide: Published Images

Deploy and run the CockroachDB-to-PostgreSQL migration tool from its published Docker images. No other repository documentation is required.

## Images

| Image | Purpose | Registry |
|-------|---------|----------|
| `runner-image` | Receives CockroachDB changefeed webhooks, writes rows into PostgreSQL | GHCR (primary), Quay (mirror) |
| `verify-image` | Compares source and destination data to confirm migration correctness | GHCR (primary), Quay (mirror) |

See [Image References](image-references.md) for pull commands, tag format, and authentication.

## Operator workflow

```
 ┌─────────────────┐    ┌──────────────────────┐    ┌─────────────────┐
 │ 1. Source setup  │───▶│ 2. Destination grants│───▶│ 3. Run runner    │
 │ (CockroachDB)    │    │ (PostgreSQL)          │    │                  │
 └─────────────────┘    └──────────────────────┘    └────────┬─────────┘
                                                            │
                                              changefeeds  │
                                              deliver data ▼
                                                            │
                                                         ┌──┴───┐
                                                         │ 4. Run│
                                                         │  verify│
                                                         │  job  │
                                                         └──────┘
```

1. **[Prepare source CockroachDB](source-setup/cockroachdb-setup.md)** — Enable rangefeeds, capture cursors, create changefeeds.
2. **[Grant destination PostgreSQL permissions](destination-setup/postgresql-grants.md)** — Give the runtime role access to databases, schemas, and tables.
3. **[Configure and start the runner](runner/getting-started.md)** — Write config, validate, and run.
4. **[Configure and start the verify-service](verify/getting-started.md)** — Write config, validate, and run.
5. **[Run a verify job](verify/job-lifecycle.md)** — Start, poll, and stop verify jobs.

> **Order matters:** Create changefeeds and destination grants before starting the runner. CockroachDB retries webhook deliveries, so changefeeds can be created while the runner is not yet running — but data will not flow until the runner is reachable.

## All pages

| Page | What it covers |
| --- | --- |
| [Image References](image-references.md) | GHCR pull commands, tag format, Quay mirror, running containers |
| [Source Setup](source-setup.md) | CockroachDB rangefeed, cursor, and changefeed setup (overview) |
| [CockroachDB setup](source-setup/cockroachdb-setup.md) | Full walkthrough: SQL, sink URL encoding, checklist |
| [Destination Grants](destination-grants.md) | PostgreSQL permissions overview |
| [PostgreSQL grants](destination-setup/postgresql-grants.md) | Per-database SQL, worked examples, checklist |
| [Runner](runner.md) | Runner overview and sub-page index |
| [Runner getting started](runner/getting-started.md) | Pull, configure, validate, and run the runner |
| [Runner configuration](runner/configuration.md) | Full YAML reference |
| [Runner endpoints](runner/endpoints.md) | `/healthz`, `/metrics`, `/ingest/{mapping_id}` |
| [Verify Service](verify-service.md) | Verify-service overview and sub-page index |
| [Verify getting started](verify/getting-started.md) | Pull, configure, validate, and run the verify-service |
| [Verify configuration](verify/configuration.md) | Full YAML reference |
| [Verify job lifecycle](verify/job-lifecycle.md) | Start, poll, and stop verify jobs |
| [TLS reference](tls-reference.md) | TLS settings for all components |
| [Troubleshooting](troubleshooting.md) | Common failures and fixes |

=== END FILE ===

=== FILE: docs/public_image_operator_guide/runner/configuration.md ===
# Runner: Configuration Reference

The runner reads a single YAML configuration file. Pass its path with `--config <PATH>`.

## Top-level structure

```yaml
webhook: ...
reconcile: ...
mappings:
  - id: ...
    source: ...
    destination: ...
```

All three top-level keys (`webhook`, `reconcile`, `mappings`) are required.

## `webhook`

Controls the HTTPS/HTTP listener that receives CockroachDB changefeed webhook batches.

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `bind_addr` | string | yes | Host and port to bind, e.g. `0.0.0.0:8443` or `127.0.0.1:8080` |
| `mode` | string | no | `http` or `https`. Defaults to `https` if omitted. |
| `tls` | object | yes when `mode: https` | TLS configuration. Must not appear when `mode: http`. |

### `webhook.tls`

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `cert_path` | path | yes | Server certificate file path |
| `key_path` | path | yes | Server private key file path |
| `client_ca_path` | path | no | CA certificate to require and verify client certificates (mTLS). Omit for plain HTTPS. |

### Examples

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

> **Rules:** When `mode: https`, the `tls` block is required with at least `cert_path` and `key_path`. When `mode: http`, the `tls` block must not appear. `mode` defaults to `https` if omitted.

## `reconcile`

Controls how often the runner performs a reconciliation pass over the destination tables.

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `interval_secs` | integer | yes | Seconds between reconciliation passes. Must be greater than zero. |

```yaml
reconcile:
  interval_secs: 30
```

## `mappings`

A list of one or more mapping objects. Each mapping ties one source CockroachDB database and set of tables to one destination PostgreSQL database.

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `id` | string | yes | Stable identifier used in the ingest route `/ingest/<id>`. Must be unique across all mappings. |
| `source` | object | yes | Source CockroachDB database and tables. |
| `destination` | object | yes | Destination PostgreSQL connection. |

### `source`

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `database` | string | yes | Source CockroachDB database name. |
| `tables` | list of strings | yes | Schema-qualified table names, e.g. `public.customers`. Must contain at least one entry. Must be unique within a mapping. |

```yaml
source:
  database: demo_a
  tables:
    - public.customers
    - public.orders
```

### `destination`

The destination can be specified in two **mutually exclusive** forms: **URL** or **decomposed fields**. Never mix them.

#### URL form

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a
```

For TLS connections, add `sslmode` and related query parameters:

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt&sslcert=/config/certs/destination-client.crt&sslkey=/config/certs/destination-client.key
```

> **Rule:** The `url` field cannot be mixed with `host`, `port`, `database`, `user`, `password`, or `tls` fields.

#### Decomposed form

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `host` | string | yes | PostgreSQL hostname or IP. Unix sockets are not supported. |
| `port` | integer | yes | PostgreSQL port. |
| `database` | string | yes | Target database name. |
| `user` | string | yes | Database user. |
| `password` | string | yes | Database password. |
| `tls` | object | no | TLS configuration for the destination connection. |

```yaml
destination:
  host: pg-a.example.internal
  port: 5432
  database: app_a
  user: migration_user_a
  password: runner-secret-a
```

#### `destination.tls` (decomposed form only)

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `mode` | string | yes | One of: `require`, `verify-ca`, `verify-full` |
| `ca_cert_path` | path | required when `mode` is `verify-ca` or `verify-full` | CA certificate to verify the server certificate |
| `client_cert_path` | path | no | Client certificate for mTLS to the destination |
| `client_key_path` | path | no | Client private key. Must appear together with `client_cert_path`. |

| `mode` | Behavior | `ca_cert_path` required |
| ------ | -------- | ----------------------- |
| `require` | TLS without verifying server cert | no |
| `verify-ca` | TLS with server cert verified against CA | yes |
| `verify-full` | TLS with server cert and hostname verified | yes |

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

## CLI reference

| Command | Required flags | Optional flags |
| ------- | -------------- | -------------- |
| `validate-config` | `--config <PATH>` | `--deep`, `--log-format text\|json` |
| `run` | `--config <PATH>` | `--log-format text\|json` |

- `validate-config` without `--deep` is offline — it checks config structure and field values.
- `validate-config --deep` additionally verifies each destination database is reachable and that every mapped table exists.
- `--log-format json` outputs structured JSON logs on stderr. Default is `text`.

## Full example

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

## See also

- [Runner getting started](./getting-started.md) — pull, configure, validate, and run
- [Runner endpoints](./endpoints.md) — `/healthz`, `/metrics`, `/ingest/{mapping_id}`
- [TLS reference](../tls-reference.md) — detailed TLS configuration for all components

=== END FILE ===

=== FILE: docs/public_image_operator_guide/runner/endpoints.md ===
# Runner: HTTP Endpoints

The runner exposes three HTTP endpoints on the address configured in `webhook.bind_addr`.

## Health check

```
GET /healthz
```

Returns `200 OK` when the runner process is alive and ready.

```bash
curl -k https://runner.example.internal:8443/healthz
```

Use this for container health checks, load-balancer probes, and readiness gates. Use `http://` when `webhook.mode` is `http`.

## Metrics

```
GET /metrics
```

Returns Prometheus-formatted metrics as `text/plain`.

```bash
curl -k https://runner.example.internal:8443/metrics
```

Metrics are prefixed with `cockroach_migration_tool_runner_`.

## Ingest

```
POST /ingest/{mapping_id}
```

This is the endpoint that CockroachDB changefeeds post to. The route is exactly `/ingest/{mapping_id}`, where `{mapping_id}` matches the `id` field in a runner mapping.

CockroachDB changefeeds send batches to this endpoint automatically. You do not call it manually under normal operation.

### Request format

Content-Type: `application/json`

A row batch:

```json
{
  "length": 2,
  "payload": [
    {
      "after": {"id": 1, "email": "first@example.com"},
      "key": {"id": 1},
      "op": "c",
      "source": {"database_name": "demo_a", "schema_name": "public", "table_name": "customers"}
    },
    {
      "key": {"id": 2},
      "op": "d",
      "source": {"database_name": "demo_a", "schema_name": "public", "table_name": "customers"}
    }
  ]
}
```

A resolved watermark:

```json
{"resolved": "1776526353000000000.0000000000"}
```

### Response codes

| Status | Meaning |
| ------ | ------- |
| `200 OK` | Batch accepted |
| `400 Bad Request` | Malformed batch (e.g. length mismatch) |
| `404 Not Found` | Unknown `mapping_id` |
| `500 Internal Server Error` | Processing failure |

### Manual test

You can send a test batch to verify the ingest path is wired end to end:

```bash
curl -k -X POST \
  -H 'content-type: application/json' \
  -d '{"length":1,"payload":[{"after":{"id":1,"name":"test"},"key":{"id":1},"op":"c","source":{"database_name":"demo_a","schema_name":"public","table_name":"customers"}}]}' \
  https://localhost:8443/ingest/app-a
```

A `200` response confirms the runner received the batch for the `app-a` mapping.

## See also

- [Runner getting started](./getting-started.md) — pull, configure, validate, and run
- [Runner configuration](./configuration.md) — full YAML reference
- [CockroachDB source setup](../source-setup/cockroachdb-setup.md) — changefeeds that target `/ingest/{mapping_id}`
- [Troubleshooting](../troubleshooting.md) — common runner errors

=== END FILE ===

=== FILE: docs/public_image_operator_guide/runner/getting-started.md ===
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

=== END FILE ===

=== FILE: docs/public_image_operator_guide/runner.md ===
# Runner

The runner image receives CockroachDB changefeed webhooks over HTTPS and writes the incoming row mutations into PostgreSQL destination tables.

## Sub-pages

| Page | What it covers |
| --- | --- |
| [Getting started](./runner/getting-started.md) | Pull the image, write config, validate, and run |
| [Configuration reference](./runner/configuration.md) | Full YAML reference for runner config |
| [HTTP endpoints](./runner/endpoints.md) | `/healthz`, `/metrics`, `/ingest/{mapping_id}` |

## Quick start

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/djosh34/runner-image:<git-sha> \
  run --config /config/runner.yml
```

For step-by-step instructions including validation and Docker Compose, see [Getting started](./runner/getting-started.md).

## CLI summary

```
runner [--log-format text|json] validate-config --config <PATH> [--deep]
runner [--log-format text|json] run --config <PATH>
```

- `validate-config` — offline structural check. Add `--deep` to also test destination connectivity.
- `run` — start the webhook listener and reconciliation loop.

## Key endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/healthz` | Liveness probe (`200 OK`) |
| `GET` | `/metrics` | Prometheus metrics |
| `POST` | `/ingest/{mapping_id}` | Changefeed webhook sink target |

See [HTTP endpoints](./runner/endpoints.md) for request/response details.

## Related pages

- [Source setup](./source-setup/cockroachdb-setup.md) — CockroachDB changefeeds that target `/ingest/{mapping_id}`
- [Destination grants](./destination-setup/postgresql-grants.md) — PostgreSQL permissions the runner needs
- [TLS reference](./tls-reference.md) — TLS configuration for the webhook listener and destination connections
- [Troubleshooting](./troubleshooting.md) — Common runner errors and fixes

=== END FILE ===

=== FILE: docs/public_image_operator_guide/source-setup/cockroachdb-setup.md ===
# CockroachDB Source Setup

You must prepare the source CockroachDB cluster before starting the runner. The runner only receives webhook payloads — it does not create changefeeds for you.

## What you need

- CockroachDB SQL access with permission to change cluster settings and create changefeeds.
- The externally reachable HTTPS URL for the runner webhook (e.g. `runner.example.internal:8443`).
- The CA certificate (PEM) that CockroachDB will trust when posting to the runner webhook.
- The mapping IDs and source tables from your runner configuration.
- A chosen resolved timestamp interval (e.g. `5s`).

## Step 1: Enable rangefeeds

Run once per cluster, before any changefeed creation:

```sql
SET CLUSTER SETTING kv.rangefeed.enabled = true;
```

This is a cluster-wide setting. It persists across restarts and only needs to be run once.

## Step 2: Capture a cursor per source database

Run this once per source database, immediately before creating changefeeds for that database:

```sql
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;
```

The result is a decimal value like `1745877420457561000.0000000000`. Paste this exact value into every `CREATE CHANGEFEED` statement for that database.

> **Important:** Capture the cursor once per database and reuse it across all mappings that share that database. This keeps changefeeds aligned on the same start boundary.

## Step 3: Create one changefeed per mapping

For each mapping in the runner config, create one changefeed:

```sql
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders
INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D'
WITH cursor = '1745877420457561000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';
```

### Required changefeed options

| Option | Value | Why |
| ------ | ---- | --- |
| `cursor` | Decimal from `cluster_logical_timestamp()` | Starts the changefeed at a consistent point |
| `initial_scan` | `'yes'` | Snapshots existing data before live streaming |
| `envelope` | `'enriched'` | The webhook payload format the runner expects |
| `resolved` | Interval such as `'5s'` | How often resolved watermarks are emitted |

### Sink URL format

```
webhook-<base_url>/ingest/<mapping_id>?ca_cert=<percent-encoded-base64-cert>
```

| Component | What to substitute |
| --------- | ------------------ |
| `<base_url>` | Externally reachable runner URL, e.g. `https://runner.example.internal:8443` |
| `<mapping_id>` | The `id` field from the corresponding mapping in the runner config |
| `<percent-encoded-base64-cert>` | PEM-encoded CA cert, base64-encoded with no line breaks, then percent-encoded |

> A trailing slash on the base URL is normalized automatically, but prefer omitting it for clarity.

### Encoding the CA certificate

```bash
CA_CERT_B64=$(cat /config/certs/ca.crt | base64 -w0 | python3 -c 'import urllib.parse,sys; print(urllib.parse.quote(sys.stdin.read().strip()))')
echo "ca_cert=${CA_CERT_B64}"
```

### HTTP (non-TLS) sinks

When `webhook.mode` is `http` in the runner config, use an HTTP sink URL and omit `ca_cert`:

```sql
CREATE CHANGEFEED FOR TABLE demo_a.public.customers
INTO 'webhook-http://runner.example.internal:8080/ingest/app-a'
WITH
  cursor = '1745877420457561000.0000000000',
  initial_scan = 'yes',
  envelope = 'enriched',
  resolved = '5s';
```

> **Warning:** Use HTTP only in isolated, trusted networks. Production deployments should always use HTTPS.

## Worked example: two databases, two mappings

```sql
-- Connect to: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require

-- Step 1: Enable rangefeeds (once per cluster)
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

## Order of operations

1. Enable rangefeed on the CockroachDB cluster.
2. Create destination PostgreSQL grants (see [PostgreSQL destination grants](../destination-setup/postgresql-grants.md)).
3. Capture the changefeed cursor (`cluster_logical_timestamp()`).
4. Create the changefeeds pointing at the runner's `/ingest/{mapping_id}` endpoint.
5. Start the runner so it begins listening on `/ingest/{mapping_id}`.

CockroachDB retries webhook deliveries, so changefeeds can be created before the runner is running — but data will not flow until the runner is reachable.

## Checklist

- [ ] `kv.rangefeed.enabled = true` is set on the source cluster.
- [ ] One cursor captured per source database before creating changefeeds.
- [ ] Every `CREATE CHANGEFEED` uses `cursor`, `initial_scan = 'yes'`, `envelope = 'enriched'`, and `resolved`.
- [ ] Each sink URL ends with `/ingest/<mapping_id>` and the mapping ID matches the runner config.
- [ ] Table names in the changefeed are fully qualified as `database.schema.table`.
- [ ] The `ca_cert` query parameter contains properly percent-encoded base64 certificate data.
- [ ] The runner HTTPS endpoint is reachable from the source cluster.

## See also

- [Runner getting started](../runner/getting-started.md) — write runner config and start the runner
- [TLS reference](../tls-reference.md) — runner webhook TLS configuration
- [Troubleshooting](../troubleshooting.md) — common source and changefeed errors

=== END FILE ===

=== FILE: docs/public_image_operator_guide/source-setup.md ===
# Source CockroachDB Setup

Before starting the runner, you must prepare the source CockroachDB cluster. The runner receives webhook payloads — it does not create changefeeds for you.

## Three steps

1. **Enable rangefeeds** — cluster-wide setting, run once per cluster.
2. **Capture a cursor** — one `cluster_logical_timestamp()` per source database.
3. **Create changefeeds** — one `CREATE CHANGEFEED` per mapping, each targeting the runner's `/ingest/{mapping_id}` endpoint.

## Detailed guide

The full walkthrough with SQL examples, sink URL encoding, and a pre-flight checklist:

**[CockroachDB source setup](./source-setup/cockroachdb-setup.md)**

## Quick reference

```sql
-- 1. Enable rangefeeds (once per cluster)
SET CLUSTER SETTING kv.rangefeed.enabled = true;

-- 2. Capture cursor (once per source database)
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;

-- 3. Create changefeed (one per mapping)
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders
  INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=PERCENT_ENCODED_BASE64_CA_CERT'
  WITH
    cursor = '1745877420457561000.0000000000',
    initial_scan = 'yes',
    envelope = 'enriched',
    resolved = '5s';
```

See the [detailed guide](./source-setup/cockroachdb-setup.md) for sink URL encoding, HTTP sink configuration, and the complete order-of-operations checklist.

=== END FILE ===

=== FILE: docs/public_image_operator_guide/tls-reference.md ===
# TLS Configuration Reference

Every TLS setting across the runner and verify-service, in one place. Use this page when configuring HTTPS listeners, mTLS, or database connections with certificate verification.

## Certificate mounting convention

Mount PEM-encoded certificates and keys under `/config/certs/...` inside containers. Config file paths should reference these mount points:

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

## Runner: webhook listener

### HTTP mode (development only)

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
```

No TLS configuration. Only suitable for trusted local networks.

### HTTPS mode

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

The server presents `server.crt` to connecting clients (CockroachDB changefeeds).

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

The server additionally verifies that connecting clients present a certificate signed by `client-ca.crt`.

> **Rules:** When `mode: https`, the `tls` block is required with at least `cert_path` and `key_path`. When `mode: http`, the `tls` block must not appear. `mode` defaults to `https` if omitted. `client_ca_path` is always optional.

See [Runner configuration](runner/configuration.md) for the full webhook field reference.

## Runner: destination connection

The runner connects from the container to PostgreSQL. Two configuration forms are available.

### URL form with sslmode query parameters

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt&sslcert=/config/certs/destination-client.crt&sslkey=/config/certs/destination-client.key
```

| `sslmode` | Behavior |
| --------- | -------- |
| `disable` | No TLS |
| `require` | TLS enabled, no server certificate verification |
| `verify-ca` | TLS enabled, server certificate verified against CA |
| `verify-full` | TLS enabled, server certificate and hostname verified |

### Decomposed form with explicit tls block

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

| `mode` | Behavior | `ca_cert_path` required |
| ------ | -------- | ----------------------- |
| `require` | TLS without verifying server cert | no |
| `verify-ca` | TLS with server cert verified against CA | yes |
| `verify-full` | TLS with server cert and hostname verified | yes |

> **Rules:** The URL form and decomposed form are mutually exclusive. `client_cert_path` and `client_key_path` must always appear together. When `mode` is `verify-ca` or `verify-full`, `ca_cert_path` is required.

See [Runner configuration](runner/configuration.md) for the full destination field reference.

## Verify-service: listener

### HTTP listener

```yaml
listener:
  bind_addr: 0.0.0.0:8080
```

### HTTPS listener

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

### mTLS listener

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

> **Rules:** When `listener.tls` is present, `cert_path` and `key_path` are both required. When `listener.tls` is omitted, the listener serves plain HTTP. `client_ca_path` is optional; when present, callers must present a client certificate signed by this CA.

See [Verify configuration](verify/configuration.md) for the full listener field reference.

## Verify-service: database connections

Both `verify.source` and `verify.destination` use the same `url` plus `tls` block shape.

### Source with verify-full and client certificates

```yaml
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
```

### Destination with verify-ca (CA only)

```yaml
verify:
  destination:
    url: postgresql://verify_target:secret@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
```

### Source with passwordless client certificate auth

```yaml
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
```

> **Rules:** When `sslmode=verify-ca` or `sslmode=verify-full` appears in the URL, `ca_cert_path` is required in the `tls` block. `client_cert_path` and `client_key_path` must always appear as a pair.

See [Verify configuration](verify/configuration.md) for the full database connection field reference.

## Quick reference: TLS component mapping

| Component | Config path | TLS fields |
| --------- | ----------- | ---------- |
| Runner webhook listener | `webhook.mode`, `webhook.tls.*` | `cert_path`, `key_path`, `client_ca_path` (optional) |
| Runner destination (URL form) | `mappings[].destination.url` | `sslmode`, `sslrootcert`, `sslcert`, `sslkey` in query params |
| Runner destination (decomposed form) | `mappings[].destination.tls.*` | `mode`, `ca_cert_path`, `client_cert_path`, `client_key_path` |
| Verify listener | `listener.tls.*` | `cert_path`, `key_path`, `client_ca_path` (optional) |
| Verify source/destination | `verify.source.tls.*`, `verify.destination.tls.*` | `ca_cert_path`, `client_cert_path`, `client_key_path` |

## See also

- [Runner configuration](runner/configuration.md) — full runner YAML reference
- [Verify configuration](verify/configuration.md) — full verify-service YAML reference
- [Troubleshooting](troubleshooting.md) — common TLS-related errors

=== END FILE ===

=== FILE: docs/public_image_operator_guide/troubleshooting.md ===
# Troubleshooting

Common setup failures and how to diagnose them.

## Runner

### `validate-config` fails locally

**Symptom:** `docker run ... validate-config --config /config/runner.yml` exits nonzero with a parse or validation error.

**Checks:**

- All three top-level keys exist: `webhook`, `reconcile`, `mappings`.
- `webhook.bind_addr` is a valid `host:port` string, e.g. `0.0.0.0:8443`.
- When `webhook.mode` is `https`, the `webhook.tls` block with `cert_path` and `key_path` is present.
- When `webhook.mode` is `http`, there is no `webhook.tls` block.
- `reconcile.interval_secs` is a positive integer.
- `mappings` contains at least one entry.
- Each mapping has a unique `id`.
- Each `mappings[].source.tables` entry is schema-qualified (e.g. `public.customers`, not `customers`).
- `mappings[].destination` is either a `url` string alone or decomposed fields (`host`, `port`, `database`, `user`, `password`) optionally with `tls` — never both.
- When using the decomposed `tls` block with `mode: verify-ca` or `mode: verify-full`, `ca_cert_path` is set.
- `client_cert_path` and `client_key_path` always appear together.

> See [Runner configuration](runner/configuration.md) for the full config reference.

### `validate-config --deep` cannot reach destination

**Symptom:** Connection refused or timeout when validating destination connectivity.

**Checks:**

- The Docker container can reach the destination host and port. Use `--network host` if the database is on the host network.
- The destination PostgreSQL accepts connections from the runner container's IP.
- `sslmode` is correct. If the database requires TLS, use `verify-ca` or `verify-full` and mount the CA certificate.
- If using client certificates, both `client_cert_path` and `client_key_path` are mounted in the container and referenced correctly.

> See [PostgreSQL destination grants](destination-setup/postgresql-grants.md) for the required permissions.

### Runner starts but changefeeds get connection refused

**Symptom:** CockroachDB changefeeds fail to post to the runner webhook.

**Checks:**

- The runner is listening on an address reachable from the CockroachDB cluster. `0.0.0.0` is the container default; `127.0.0.1` is only reachable locally.
- The CockroachDB cluster can resolve the runner hostname and reach the port.
- The `ca_cert` in the changefeed sink URL is the CA that signed the runner's server certificate, properly percent-encoded.
- The sink URL uses `webhook-https://` (not `webhook-http://`) when the runner is in HTTPS mode.

> See [CockroachDB source setup](source-setup/cockroachdb-setup.md) for sink URL format and encoding.

### Runner returns 404 Unknown Mapping

**Symptom:** `POST /ingest/<mapping_id>` returns `404`.

**Checks:**

- The `mapping_id` in the changefeed sink URL exactly matches the `id` field in the runner config (case-sensitive).
- The runner was restarted after the mapping was added to the config.

### Runner returns 400 Bad Request

**Symptom:** `POST /ingest/<mapping_id>` returns `400`.

**Checks:**

- The `length` field in the request body matches the number of entries in the `payload` array.
- All required fields are present in each payload entry: `key`, `op`, `source` (with `database_name`, `schema_name`, `table_name`), and `after` for `c`, `u`, `r` operations.

## Verify-service

### `verify-service validate-config` fails

**Symptom:** Config validation exits nonzero.

**Checks:**

- Both `listener` and `verify` keys are present at the top level.
- `listener.bind_addr` is a valid `host:port` string.
- If `listener.tls` is present, both `cert_path` and `key_path` are set.
- `verify.source.url` and `verify.destination.url` use the `postgres://` or `postgresql://` scheme.
- When `sslmode=verify-ca` or `sslmode=verify-full` is in the URL, the corresponding `tls.ca_cert_path` is set.
- `client_cert_path` and `client_key_path` always appear as a pair.

> See [Verify configuration](verify/configuration.md) for the full config reference.

### 409 Conflict when starting a job

**Symptom:** `POST /jobs` returns `409 Conflict` with `job_already_running`.

**Cause:** A verify job is already running. Only one job can run at a time.

**Fix:** Either poll `GET /jobs/{job_id}` until the current job finishes, or stop it with `POST /jobs/{job_id}/stop`.

> See [Verify job lifecycle](verify/job-lifecycle.md) for the full job API.

### 404 Not Found for a job ID

**Symptom:** `GET /jobs/{job_id}` returns `404`.

**Cause:** The verify-service process has restarted since the job was created. Job state is in-memory and is lost on restart.

**Fix:** Start a new job.

### Job fails with source_access error

**Symptom:** Job status is `failed` with `failure.category: source_access` and `failure.code: connection_failed`.

**Checks:**

- The verify-service container can reach the source database host and port.
- The URL in `verify.source.url` is correct, including `sslmode`.
- All required TLS certificate files are mounted and the paths in `verify.source.tls` match.
- The source PostgreSQL user has permission to read the tables being verified.

### Job fails with destination_access error

**Symptom:** Same as `source_access` but for the destination.

**Checks:**

- Same connectivity checks as above, applied to `verify.destination`.

### Job succeeds but reports mismatches

**Symptom:** Job status is `failed` with `failure.category: mismatch` and `result.mismatch_summary.has_mismatches: true`.

**Action:**

1. Check `result.mismatch_summary.affected_tables` for the list of affected tables.
2. Check `result.findings` for per-row detail including `mismatching_columns`, `source_values`, and `destination_values`.
3. Decide whether to re-run verification after fixing the data, or accept the mismatches.

> See [Verify job lifecycle](verify/job-lifecycle.md) for details on reading job results.

## General

### Container cannot access mounted certificates

**Symptom:** "file not found" or "permission denied" on certificate paths.

**Fix:**

- Verify the volume mount: `docker run --rm -v "$(pwd)/config:/config:ro" ...` means local `./config` maps to `/config` inside the container.
- Verify file permissions. The container process needs read access to all mounted certificates and keys.
- Check that config paths reference the container mount target (e.g. `/config/certs/server.crt`), not the host path.

> See [TLS reference](tls-reference.md) for the certificate mounting convention.

### Port already in use

**Symptom:** Runner or verify-service fails to start with "address already in use".

**Fix:**

- Change `webhook.bind_addr` or `listener.bind_addr` to a different port.
- Or map a different host port in Docker: `-p 9443:8443` instead of `-p 8443:8443`.

### Stale image tag

**Symptom:** Unexpected behavior or missing features after pulling a new image.

**Fix:**

- Docker may cache an old layer. Use `docker pull ghcr.io/<owner>/runner-image:<git-sha>` to force a refresh.
- Verify the image digest matches the published build.

> See [Image References](image-references.md) for tag conventions.

=== END FILE ===

=== FILE: docs/public_image_operator_guide/verify/configuration.md ===
# Verify-Service: Configuration Reference

The verify-service reads a single YAML configuration file. Pass its path with `--config <path>`. The verify-image entrypoint is `molt` with default command `verify-service`; include `verify-service` explicitly when overriding `command` in Docker or Compose.

## Top-level structure

```yaml
listener: ...
verify: ...
```

Both keys are required.

## `listener`

Controls the HTTP(S) listener for the verify API.

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `bind_addr` | string | yes | Host and port, e.g. `0.0.0.0:8080` or `0.0.0.0:8443` |
| `tls` | object | no | TLS configuration. Omit for plain HTTP. |

### `listener.tls`

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `cert_path` | path | yes | Server certificate file path |
| `key_path` | path | yes | Server private key file path |
| `client_ca_path` | path | no | CA certificate to verify client certificates (mTLS). Omit for plain HTTPS. |

> **Rules:** When `listener.tls` is present, `cert_path` and `key_path` are both required. When `listener.tls` is omitted, the listener serves plain HTTP. `client_ca_path` is optional; when present, callers must present a client certificate signed by this CA.

### Examples

HTTP listener:

```yaml
listener:
  bind_addr: 0.0.0.0:8080
```

HTTPS listener:

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

## `verify`

Controls the source and destination database connections and optional features.

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `source` | object | yes | Source database connection |
| `destination` | object | yes | Destination database connection |
| `raw_table_output` | boolean | no | Enable the `POST /tables/raw` endpoint. Defaults to `false`. |

### `verify.source` and `verify.destination`

Both use the same shape:

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `url` | string | yes | PostgreSQL connection URL with `sslmode` query parameter. Must use `postgresql://` or `postgres://` scheme. |
| `tls` | object | no | File paths for TLS certificates and keys used when connecting. |

```yaml
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

### `tls` under `source` or `destination`

| Field | Type | Required | Description |
| ----- | ---- | -------- | ----------- |
| `ca_cert_path` | path | required when `sslmode` is `verify-ca` or `verify-full` | CA certificate to verify the server certificate |
| `client_cert_path` | path | no | Client certificate for mTLS |
| `client_key_path` | path | no | Client private key. Must appear together with `client_cert_path`. |

`sslmode` values in the URL:

| `sslmode` | Behavior |
| --------- | -------- |
| `disable` | No TLS |
| `require` | TLS without server certificate verification |
| `verify-ca` | TLS with CA verification (requires `ca_cert_path`) |
| `verify-full` | TLS with full verification (requires `ca_cert_path`) |

> **Rules:** When `sslmode=verify-ca` or `sslmode=verify-full` appears in `url`, `ca_cert_path` is required. `client_cert_path` and `client_key_path` must always be specified as a pair.

### Passwordless example

When using client certificate authentication, omit the password in the URL:

```yaml
source:
  url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
  tls:
    ca_cert_path: /config/certs/source-ca.crt
    client_cert_path: /config/certs/source-client.crt
    client_key_path: /config/certs/source-client.key
```

## Full example

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

## CLI reference

The verify-image default command is `verify-service`. Subcommands:

| Subcommand | Required flags | Optional flags |
| ---------- | -------------- | -------------- |
| `validate-config` | `--config <path>` | `--log-format text\|json` |
| `run` | `--config <path>` | `--log-format text\|json` |

## See also

- [Verify getting started](./getting-started.md) — pull, configure, validate, and run
- [Verify job lifecycle](./job-lifecycle.md) — start, poll, and stop verify jobs
- [TLS reference](../tls-reference.md) — detailed TLS configuration for all components

=== END FILE ===

=== FILE: docs/public_image_operator_guide/verify/getting-started.md ===
# Verify-Service: Getting Started

Pull the verify-service image, write its configuration, validate it, and start the API.

## 1. Pull the image

```bash
export GITHUB_OWNER=<owner>
export IMAGE_TAG=<published-commit-sha>
export VERIFY_IMAGE="ghcr.io/${GITHUB_OWNER}/verify-image:${IMAGE_TAG}"

docker pull "${VERIFY_IMAGE}"
```

## 2. Write configuration

Create `config/verify-service.yml`. The verify-image entrypoint is `molt` with default command `verify-service`; you must include `verify-service` in the subcommand when overriding `command` in Docker or Compose.

Minimal HTTP listener configuration:

```yaml
listener:
  bind_addr: 0.0.0.0:8080
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
  destination:
    url: postgresql://verify_target@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
```

HTTPS listener with optional mTLS:

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

For a complete field reference, see [Verify configuration](./configuration.md).

## 3. Validate configuration

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service validate-config --config /config/verify-service.yml
```

Add `--log-format json` to the subcommand for structured logs:

```bash
docker run --rm \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service validate-config --log-format json --config /config/verify-service.yml
```

## 4. Run

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service run --config /config/verify-service.yml
```

For HTTPS with mTLS, mount the certificates and adjust the port mapping:

```bash
docker run --rm \
  -p 9443:8443 \
  -v "$(pwd)/config:/config:ro" \
  "${VERIFY_IMAGE}" \
  verify-service run --log-format json --config /config/verify-service.yml
```

The service binds to the address specified in `listener.bind_addr`. When running in a container, set `bind_addr` to `0.0.0.0:<port>` and map the port with `-p`.

### CLI reference

The entrypoint is `verify-service`. Subcommands:

| Subcommand | Required flags | Optional flags |
| ---------- | -------------- | -------------- |
| `validate-config` | `--config <path>` | `--log-format text\|json` |
| `run` | `--config <path>` | `--log-format text\|json` |

## 5. Confirm it is running

```bash
curl -k https://localhost:9443/metrics
```

For HTTP:

```bash
curl http://localhost:8080/metrics
```

## 6. Start a verify job

See [Verify job lifecycle](./job-lifecycle.md) for the full job workflow. The quick version:

```bash
export VERIFY_API="https://localhost:9443"

# Start
JOB_ID=$(curl --silent --show-error --insecure \
  -H 'content-type: application/json' \
  -d '{}' \
  "${VERIFY_API}/jobs" | jq -r '.job_id')

# Poll
curl --silent --show-error --insecure "${VERIFY_API}/jobs/${JOB_ID}"

# Stop (if needed)
curl --silent --show-error --insecure \
  -H 'content-type: application/json' \
  -d '{}' \
  -X POST "${VERIFY_API}/jobs/${JOB_ID}/stop"
```

## Docker Compose

Save as `verify.compose.yml`:

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

> **Note:** The verify-image entrypoint is `molt` with default command `verify-service`. When overriding `command` in Compose, include `verify-service` explicitly — otherwise the container would execute `molt run ...` instead of `molt verify-service run ...`.

## See also

- [Verify configuration](./configuration.md) — full YAML reference
- [Verify job lifecycle](./job-lifecycle.md) — start, poll, and stop verify jobs
- [TLS reference](../tls-reference.md) — TLS configuration for the listener and database connections
- [Troubleshooting](../troubleshooting.md) — common verify-service errors

=== END FILE ===

=== FILE: docs/public_image_operator_guide/verify/job-lifecycle.md ===
# Verify-Service: Job Lifecycle

The verify-service exposes an HTTP API to start, poll, and stop verification jobs. This page describes every endpoint and the job state machine.

## Key constraints

> **Only one job** can run at a time. Starting a second returns `409 Conflict`.
>
> **Only the most recent completed job** is retained. Starting a new job evicts previous results.
>
> **Job state is in-memory.** If the process restarts, all job IDs return `404 Not Found`.

## Endpoints

### Start a verify job

```
POST /jobs
Content-Type: application/json
```

Request body (optional filters):

```json
{
  "include_schema": "^public$",
  "include_table": "^(accounts|orders)$"
}
```

All fields are optional POSIX regular expressions:

| Field | Description |
| ----- | ----------- |
| `include_schema` | Include schemas matching this regex |
| `include_table` | Include tables matching this regex |
| `exclude_schema` | Exclude schemas matching this regex |
| `exclude_table` | Exclude tables matching this regex |

To verify everything, send an empty object:

```json
{}
```

**Success —** `202 Accepted`:

```json
{"job_id": "job-000001", "status": "running"}
```

**Already running —** `409 Conflict`:

```json
{"error": {"category": "job_state", "code": "job_already_running", "message": "a verify job is already running"}}
```

### Poll job status

```
GET /jobs/{job_id}
```

```bash
curl --silent --show-error --insecure "${VERIFY_API}/jobs/${JOB_ID}"
```

**While running —** `200 OK`:

```json
{"job_id": "job-000001", "status": "running"}
```

**On success —** `200 OK`:

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

**On failure — mismatches detected** — `200 OK`:

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

**On failure — source access error** — `200 OK`:

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

**Job not found —** `404 Not Found`:

```json
{"error": {"category": "job_state", "code": "job_not_found", "message": "job not found"}}
```

### Stop a running job

```
POST /jobs/{job_id}/stop
Content-Type: application/json
```

The request body must be an empty JSON object: `{}`

```bash
curl --silent --show-error --insecure \
  -H 'content-type: application/json' \
  -d '{}' \
  -X POST "${VERIFY_API}/jobs/${JOB_ID}/stop"
```

Immediate response — `200 OK`:

```json
{"job_id": "job-000001", "status": "stopping"}
```

> The job transitions from `stopping` to `stopped` asynchronously. Poll `GET /jobs/{job_id}` until `status` is `stopped`.

### Metrics

```
GET /metrics
```

Returns Prometheus-formatted metrics as `text/plain`.

```bash
curl --silent --show-error --insecure "${VERIFY_API}/metrics"
```

Metrics are prefixed with `cockroach_migration_tool_verify_`.

## Job states

| Status | Meaning | Terminal |
| ------ | ------- | -------- |
| `running` | Job is actively verifying | no |
| `stopping` | Stop requested, winding down | no |
| `succeeded` | Verification completed, no mismatches | yes |
| `failed` | Verification completed with mismatches or an error | yes |
| `stopped` | Job was cancelled by operator | yes |

## Interpreting results

1. Check `result.summary.has_mismatches` first.
2. If `true`, inspect `result.mismatch_summary.affected_tables` for the list of affected tables.
3. For each affected table, check `result.findings` for per-row detail including `mismatching_columns`, `source_values`, and `destination_values`.

## Error categories

| Category | When it occurs |
| -------- | -------------- |
| `request_validation` | Invalid filter, unknown field, or body too large |
| `job_state` | Job already running, job not found |
| `source_access` | Cannot connect to source database |
| `mismatch` | Mismatches were detected during verification |
| `verify_execution` | Internal verify execution failure |

## Obsolete job IDs after restart

Because job state lives in memory, restarting the verify-service process means all previous job IDs return `404 Not Found`. There is no persistent job history.

## See also

- [Verify getting started](./getting-started.md) — pull, configure, validate, and run
- [Verify configuration](./configuration.md) — full YAML reference
- [Troubleshooting](../troubleshooting.md) — common verify-service errors

=== END FILE ===

=== FILE: docs/public_image_operator_guide/verify-service.md ===
# Verify Service

The verify-service image exposes an HTTP API for starting, polling, and stopping verification jobs that compare CockroachDB source data against PostgreSQL destination data.

## Sub-pages

| Page | What it covers |
| --- | --- |
| [Getting started](./verify/getting-started.md) | Pull the image, write config, validate, and run |
| [Configuration reference](./verify/configuration.md) | Full YAML reference for verify-service config |
| [Job lifecycle](./verify/job-lifecycle.md) | Start, poll, and stop verify jobs via the HTTP API |

## Key constraints

- **Only one verify job** can run at a time. Starting a second returns `409 Conflict`.
- **Only the most recent completed job** is retained. Starting a new job evicts the previous result.
- **Job state is in-memory.** Process restarts clear all job history.

## Quick start

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/djosh34/verify-image:<git-sha> \
  verify-service run --config /config/verify-service.yml
```

For step-by-step instructions including validation and Docker Compose, see [Getting started](./verify/getting-started.md).

## CLI summary

```
verify-service validate-config --log-format text|json --config <path>
verify-service run --log-format text|json --config <path>
```

- `validate-config` — offline structural and consistency check.
- `run` — start the HTTP listener and accept verify-job requests.

> **Note:** The verify-image entrypoint is `molt` with default command `verify-service`. Both subcommands accept `--log-format` directly; it is not a global flag on the parent command.

## Job lifecycle at a glance

```bash
# Start a job
curl -X POST http://localhost:8080/jobs -H 'Content-Type: application/json' -d '{}'

# Poll status
curl http://localhost:8080/jobs/{job_id}

# Stop a running job
curl -X POST http://localhost:8080/jobs/{job_id}/stop -H 'Content-Type: application/json' -d '{}'
```

See [Job lifecycle](./verify/job-lifecycle.md) for full endpoint details, filter options, and response schemas.

## Related pages

- [Runner](./runner.md) — the component that writes data into PostgreSQL destinations
- [TLS reference](./tls-reference.md) — TLS configuration for the listener and database connections
- [Troubleshooting](./troubleshooting.md) — common verify-service errors and fixes

=== END FILE ===

