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
    host: source.internal
    port: 26257
    username:
      env_ref: VERIFY_SOURCE_USERNAME
    password:
      secret_file: /config/secrets/source-password
    sslmode: verify-full
    tls:                        # optional
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt   # optional (mTLS)
      client_key_path: /config/certs/source-client.key     # optional (mTLS)
  destination:
    host: destination.internal
    port: 5432
    username: verify_target
    password:
      value: verify-target-password
    sslmode: verify-ca
    tls:                        # optional
      ca_cert_path: /config/certs/destination-ca.crt
  databases:
    - name: app
      source_database: appdb
      destination_database: appdb
```

### Top-level fields

| Key | Type | Required | Default | Purpose |
|-----|------|----------|---------|---------|
| `listener` | object | yes | — | HTTP(S) listener for the job API and metrics |
| `verify` | object | yes | — | Shared connection defaults plus named database mappings for row-level comparison |

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
| `source` | object | no | — | Default source connection settings shared by one or more mappings |
| `destination` | object | no | — | Default destination connection settings shared by one or more mappings |
| `databases` | list of objects | yes | — | Named database mappings to verify |
| `raw_table_output` | boolean | no | `false` | Enable `POST /tables/raw` for diagnostic row reads |

#### `verify.source`, `verify.destination`, and per-database endpoint blocks

Both use the same shape:

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `host` | string | yes after merge | — | Database hostname |
| `port` | integer | yes after merge | — | Database port |
| `database` | string | yes for fully specified per-database blocks | — | Database name |
| `username` | string or object | yes after merge | — | Database username credential. Scalar strings are shorthand for `{ value: ... }`. |
| `password` | string or object | no | — | Optional database password credential. Supports the same schema as `username`. |
| `sslmode` | string | yes after merge | — | `disable`, `require`, `verify-ca`, or `verify-full` |
| `tls` | object | no | — | Certificate file paths for TLS |

Credential objects must set exactly one source:

| Field | Type | Meaning |
|-------|------|---------|
| `value` | string | Use the literal credential value directly |
| `env_ref` | string | Read the credential from the named environment variable |
| `secret_file` | path | Read the credential from a local file, trimming one trailing newline |

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

#### `verify.databases[]`

| Field | Type | Required | Default | Purpose |
|-------|------|----------|---------|---------|
| `name` | string | yes | — | Stable configured database name used by the API |
| `source_database` | string | yes when shared defaults are used | — | Source database name for this mapping |
| `destination_database` | string | yes when shared defaults are used | — | Destination database name for this mapping |
| `source` | object | no | — | Per-database source overrides or a fully specified source block |
| `destination` | object | no | — | Per-database destination overrides or a fully specified destination block |

Every `verify.databases[]` entry must be an object. Scalar entries such as `- app` are rejected.

### Supported verify-service shapes

1. Shared defaults with per-database names only.
2. Shared defaults with per-database overrides using any mix of scalar values, `value`, `env_ref`, and `secret_file`.
3. No shared defaults, where every `verify.databases[]` entry supplies full `source` and `destination` blocks including `database`.

**Structured connection fields only.** The verify-service no longer accepts operator-facing raw `url` fields. It builds PostgreSQL connection strings internally from structured fields.

**`raw_table_output`.** Enable `verify.raw_table_output: true` to allow raw row reads via `POST /tables/raw`. This is useful for diagnostics but exposes table contents to any caller that can reach the verify-service API. Disabled by default.

**Job database selection.** `POST /jobs` accepts a `database` field naming one configured mapping. When multiple mappings are configured, omitting `database` is rejected.

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
