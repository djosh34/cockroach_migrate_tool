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
