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