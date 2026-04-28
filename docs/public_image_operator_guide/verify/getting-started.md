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

Create `config/verify-service.yml`. The verify image's entrypoint is `molt` and its default command is `verify-service`; you must include `verify-service` in the subcommand when overriding `command` in Docker or Compose.

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

The verify image's entrypoint is `molt` and its default command is `verify-service`. Subcommands:

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

> **Note:** The verify image's entrypoint is `molt` and its default command is `verify-service`. When overriding `command` in Compose, include `verify-service` explicitly — otherwise the container would execute `molt run ...` instead of `molt verify-service run ...`.

## See also

- [Verify configuration](./configuration.md) — full YAML reference
- [Verify job lifecycle](./job-lifecycle.md) — start, poll, and stop verify jobs
- [TLS reference](../tls-reference.md) — TLS configuration for the listener and database connections
- [Troubleshooting](../troubleshooting.md) — common verify-service errors
