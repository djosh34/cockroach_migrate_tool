# Getting Started

This guide shows the shortest Docker-oriented path and then explains the configs and commands involved.

## Prerequisites

- Docker with Compose support.
- Access to the source CockroachDB cluster.
- Access to each PostgreSQL-compatible destination database.
- TLS material for public webhook endpoints when using HTTPS or mTLS.
- Destination tables already created with the schema expected by the migration.

The runner validates destination table metadata at startup and during `validate-config --deep`. It expects configured source table names to be schema-qualified, for example `public.customers`.

## Build Images Locally

From the repository root:

```sh
docker build -t cockroach-migrate-runner:local -f Dockerfile .
docker build -t cockroach-migrate-setup-sql:local -f crates/setup-sql/Dockerfile .
docker build -t cockroach-migrate-verify:local -f cockroachdb_molt/molt/Dockerfile cockroachdb_molt/molt
```

The Rust images produce static Linux binaries for `amd64` and `arm64` Docker targets. The verify image builds the vendored Go Molt binary and starts at the `verify-service` command.

## Directory Layout For Compose

The shipped Compose files expect a working directory with a `config/` directory next to the compose file. One simple local layout is:

```text
config/
  cockroach-setup.yml
  postgres-grants.yml
  runner.yml
  verify-service.yml
  ca.crt
  certs/
    server.crt
    server.key
    client-ca.crt
    source-ca.crt
    source-client.crt
    source-client.key
    destination-ca.crt
    destination-client.crt
    destination-client.key
```

The example compose files mount these files as Docker configs under `/config`.

## Runner Config

`runner` has two commands:

```sh
runner validate-config --config config/runner.yml
runner validate-config --config config/runner.yml --deep
runner run --config config/runner.yml
```

Use `--deep` when the destination database is reachable. It connects to destinations and checks that configured tables can be loaded from the catalog.

Minimal HTTPS runner config:

```yaml
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
      host: postgres
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
```

Destination can also be expressed as a URL instead of decomposed fields:

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@postgres:5432/app_a?sslmode=verify-full&sslrootcert=/config/certs/destination-ca.crt
```

Or with decomposed TLS fields:

```yaml
destination:
  host: postgres
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

Supported destination TLS modes are `require`, `verify-ca`, and `verify-full`. `verify-ca` and `verify-full` require `ca_cert_path`. Client certificate and key paths must be set together.

For local non-TLS testing only, the webhook listener can run as HTTP:

```yaml
webhook:
  bind_addr: 127.0.0.1:8443
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
      host: localhost
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
```

If `webhook.mode` is omitted, it defaults to HTTPS and requires `webhook.tls`.

## Start Runner With Compose

```sh
RUNNER_IMAGE=cockroach-migrate-runner:local \
RUNNER_HTTPS_PORT=8443 \
docker compose -f artifacts/compose/runner.compose.yml up
```

The compose file runs:

```sh
runner run --log-format json --config /config/runner.yml
```

The runner exposes:

- `GET /healthz`: returns `ok`.
- `GET /metrics`: Prometheus text metrics.
- `POST /ingest/{mapping_id}`: CockroachDB webhook sink target for changefeed events.

## Source CockroachDB Setup SQL

`setup-sql emit-cockroach-sql` reads Cockroach setup config and prints SQL. It does not connect to the database.

```yaml
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
```

Emit the SQL:

```sh
docker run --rm \
  --mount type=bind,src="$PWD/config",dst=/config,ro \
  cockroach-migrate-setup-sql:local \
  emit-cockroach-sql --config /config/cockroach-setup.yml
```

The generated SQL:

- Enables `kv.rangefeed.enabled`.
- Emits `SELECT cluster_logical_timestamp() AS changefeed_cursor;`.
- Emits one `CREATE CHANGEFEED` per mapping.
- Uses `initial_scan = 'yes'`, `envelope = 'enriched'`, `enriched_properties = 'source'`, and the configured `resolved` interval.
- Embeds the runner CA certificate into the webhook sink query string.

Before running the `CREATE CHANGEFEED` statements, replace `__CHANGEFEED_CURSOR__` with the cursor returned by `cluster_logical_timestamp()`.

The Docker Compose variant runs the Cockroach SQL emission command:

```sh
SETUP_SQL_IMAGE=cockroach-migrate-setup-sql:local \
docker compose -f artifacts/compose/setup-sql.compose.yml run --rm setup-sql
```

To emit JSON grouped by source database:

```sh
setup-sql emit-cockroach-sql --config config/cockroach-setup.yml --format json
```

## Destination PostgreSQL Grants SQL

`setup-sql emit-postgres-grants` renders the grants needed by the runner runtime role.

```yaml
mappings:
  - id: app-a
    destination:
      database: app_a
      runtime_role: migration_user_a
      tables:
        - public.customers
        - public.orders
```

Emit grants:

```sh
docker run --rm \
  --mount type=bind,src="$PWD/config",dst=/config,ro \
  cockroach-migrate-setup-sql:local \
  emit-postgres-grants --config /config/postgres-grants.yml
```

The generated SQL grants:

- `CONNECT, CREATE` on each destination database to the runtime role.
- `USAGE` on schema `public`.
- `SELECT, INSERT, UPDATE, DELETE` on each configured destination table.

The runner creates helper tables in `_cockroach_migration_tool`, so the runtime role also needs enough privilege to create and mutate objects in the destination database.

## Verify Service Config

The verify service has two commands:

```sh
molt verify-service validate-config --config config/verify-service.yml
molt verify-service run --config config/verify-service.yml
```

Inside the project verify Docker image, `molt verify-service` is already the entrypoint, so the container command starts at `validate-config` or `run`.

HTTP listener example:

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
  raw_table_output: false
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
    url: postgresql://verify_target@destination.internal:5432/appdb?sslmode=verify-ca
    tls:
      ca_cert_path: /config/certs/destination-ca.crt
      client_cert_path: /config/certs/destination-client.crt
      client_key_path: /config/certs/destination-client.key
  raw_table_output: true
```

Run with Compose:

```sh
VERIFY_IMAGE=cockroach-migrate-verify:local \
VERIFY_HTTPS_PORT=9443 \
docker compose -f artifacts/compose/verify.compose.yml up
```

The compose file maps host `${VERIFY_HTTPS_PORT:-9443}` to container port `8080`, while the service itself binds whatever `listener.bind_addr` says. Keep those ports aligned in your config and compose usage.

## Verify API Examples

Start a full verify job:

```sh
curl -sS -X POST http://localhost:8080/jobs \
  -H 'content-type: application/json' \
  -d '{}'
```

Start a filtered verify job:

```sh
curl -sS -X POST http://localhost:8080/jobs \
  -H 'content-type: application/json' \
  -d '{"include_schema":"^public$","include_table":"^(customers|orders)$"}'
```

Poll a job:

```sh
curl -sS http://localhost:8080/jobs/job-000001
```

Stop a job:

```sh
curl -sS -X POST http://localhost:8080/jobs/job-000001/stop \
  -H 'content-type: application/json' \
  -d '{}'
```

Read raw table data when `raw_table_output: true`:

```sh
curl -sS -X POST http://localhost:8080/tables/raw \
  -H 'content-type: application/json' \
  -d '{"database":"source","schema":"public","table":"customers"}'
```

`database` must be `source` or `destination`. `schema` and `table` must be simple SQL identifiers.

