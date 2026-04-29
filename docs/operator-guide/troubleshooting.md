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
