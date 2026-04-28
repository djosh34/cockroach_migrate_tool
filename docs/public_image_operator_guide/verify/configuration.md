# Verify-Service: Configuration Reference

The verify-service reads a single YAML configuration file. Pass its path with `--config <path>`. The verify image's entrypoint is `molt` and its default command is `verify-service`; include `verify-service` explicitly when overriding `command` in Docker or Compose.

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

The verify image's entrypoint is `molt` and its default command is `verify-service`. Subcommands:

| Subcommand | Required flags | Optional flags |
| ---------- | -------------- | -------------- |
| `validate-config` | `--config <path>` | `--log-format text\|json` |
| `run` | `--config <path>` | `--log-format text\|json` |

## See also

- [Verify getting started](./getting-started.md) — pull, configure, validate, and run
- [Verify job lifecycle](./job-lifecycle.md) — start, poll, and stop verify jobs
- [TLS reference](../tls-reference.md) — detailed TLS configuration for all components