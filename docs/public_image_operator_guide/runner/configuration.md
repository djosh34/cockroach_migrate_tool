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