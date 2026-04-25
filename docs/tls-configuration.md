# TLS Configuration Reference

Use this reference when you need TLS for the runner webhook, runner destination databases, the verify listener, or verify database connections.

Mount PEM-encoded certificates and keys under `/config/certs/...` for containerized deployments so the same paths work across the examples in this repository.

## Component-to-Field Mapping

| Component | TLS-relevant fields |
| --- | --- |
| Runner webhook | `mode` (`http` or `https`), `tls.cert_path`, `tls.key_path` |
| Runner destination | `tls.mode`, `tls.ca_cert_path`, `tls.client_cert_path`, `tls.client_key_path` |
| Verify listener | `tls.cert_path`, `tls.key_path`, `tls.client_ca_path` (optional for mTLS) |
| Verify source and destination | `url` with `sslmode`, `tls.ca_cert_path`, `tls.client_cert_path`, `tls.client_key_path` |

The runner webhook also supports optional `webhook.tls.client_ca_path` when you want the listener to require client certificates. That is the same mTLS role that `listener.tls.client_ca_path` serves on the verify API.

## TLS Modes

- `http` or no TLS: plain text. Use only for local development.
- `https`: the server presents a certificate and clients verify it before sending data.
- `mTLS`: both sides present certificates and both sides verify who is on the other end.
- `require`: TLS is enabled, but the client does not verify the server certificate.
- `verify-ca`: TLS is enabled and the client verifies the server certificate against a trusted CA.
- `verify-full`: TLS is enabled and the client verifies both the server certificate and the hostname.

## Common Scenarios

### Runner webhook HTTP (local development)

```yaml
webhook:
  bind_addr: 0.0.0.0:8080
  mode: http
```

Use this only on a local development network where plain text is acceptable.

### Runner webhook HTTPS (production)

```yaml
webhook:
  bind_addr: 0.0.0.0:8443
  mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

Add `webhook.tls.client_ca_path` when the webhook receiver must require client certificates.

### Runner destination with `verify-ca`

```yaml
destination:
  url: postgresql://migration_user_a:runner-secret-a@pg-a.example.internal:5432/app_a?sslmode=verify-ca&sslrootcert=/config/certs/destination-ca.crt
```

Use `verify-ca` when the runner should verify the server certificate against your CA bundle but hostname verification is not part of the deployment contract.

### Runner destination with `verify-full` and client certificates

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

Use this shape when you want hostname verification plus a client certificate on the runner's outbound database connection.

### Verify listener HTTPS

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
```

This enables server-side TLS on the verify API without requiring client certificates.

### Verify listener mTLS

```yaml
listener:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_ca_path: /config/certs/client-ca.crt
```

Add `client_ca_path` when callers must present client certificates to the verify API.

### Verify DB connection with `sslmode=verify-full`

```yaml
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full
    tls:
      ca_cert_path: /config/certs/source-ca.crt
      client_cert_path: /config/certs/source-client.crt
      client_key_path: /config/certs/source-client.key
```

Use the same `sslmode` and nested `tls` file paths on `verify.destination` when the destination database also verifies the server certificate or requires client certificates.

For the webhook payload shape, see `README.md#webhook-payload-format`.
For verify API endpoints, see `openapi/verify-service.yaml`.
