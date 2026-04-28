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