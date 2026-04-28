# Verify Service

The verify-service image exposes an HTTP API for starting, polling, and stopping verification jobs that compare CockroachDB source data against PostgreSQL destination data.

## Sub-pages

| Page | What it covers |
| --- | --- |
| [Getting started](./verify/getting-started.md) | Pull the image, write config, validate, and run |
| [Configuration reference](./verify/configuration.md) | Full YAML reference for verify-service config |
| [Job lifecycle](./verify/job-lifecycle.md) | Start, poll, and stop verify jobs via the HTTP API |

## Key constraints

- **Only one verify job** can run at a time. Starting a second returns `409 Conflict`.
- **Only the most recent completed job** is retained. Starting a new job evicts the previous result.
- **Job state is in-memory.** Process restarts clear all job history.

## Quick start

```bash
docker run --rm \
  -p 8080:8080 \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/djosh34/verify-image:<git-sha> \
  verify-service run --config /config/verify-service.yml
```

For step-by-step instructions including validation and Docker Compose, see [Getting started](./verify/getting-started.md).

## CLI summary

```
verify-service validate-config --config <path> [--log-format text|json]
verify-service run --config <path> [--log-format text|json]
```

- `validate-config` — offline structural and consistency check.
- `run` — start the HTTP listener and accept verify-job requests.

> **Note:** The verify image's entrypoint is `molt` and its default command is `verify-service`. Both subcommands accept `--log-format` directly; it is not a global flag on the parent command.

## Job lifecycle at a glance

```bash
# Start a job
curl -X POST http://localhost:8080/jobs -H 'Content-Type: application/json' -d '{}'

# Poll status
curl http://localhost:8080/jobs/{job_id}

# Stop a running job
curl -X POST http://localhost:8080/jobs/{job_id}/stop -H 'Content-Type: application/json' -d '{}'
```

See [Job lifecycle](./verify/job-lifecycle.md) for full endpoint details, filter options, and response schemas.

## See also

- [Runner](./runner.md) — the component that writes data into PostgreSQL destinations
- [TLS reference](./tls-reference.md) — TLS configuration for the listener and database connections
- [Troubleshooting](./troubleshooting.md) — common verify-service errors and fixes
