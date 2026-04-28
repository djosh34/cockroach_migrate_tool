# Runner

The runner image receives CockroachDB changefeed webhooks over HTTPS and writes the incoming row mutations into PostgreSQL destination tables.

## Sub-pages

| Page | What it covers |
| --- | --- |
| [Getting started](./runner/getting-started.md) | Pull the image, write config, validate, and run |
| [Configuration reference](./runner/configuration.md) | Full YAML reference for runner config |
| [HTTP endpoints](./runner/endpoints.md) | `/healthz`, `/metrics`, `/ingest/{mapping_id}` |

## Quick start

```bash
docker run --rm \
  -p 8443:8443 \
  -v "$(pwd)/config:/config:ro" \
  ghcr.io/djosh34/runner-image:<git-sha> \
  run --config /config/runner.yml
```

For step-by-step instructions including validation and Docker Compose, see [Getting started](./runner/getting-started.md).

## CLI summary

```
runner [--log-format text|json] validate-config --config <PATH> [--deep]
runner [--log-format text|json] run --config <PATH>
```

- `validate-config` — offline structural check. Add `--deep` to also test destination connectivity.
- `run` — start the webhook listener and reconciliation loop.

## Key endpoints

| Method | Path | Purpose |
|--------|------|---------|
| `GET` | `/healthz` | Liveness probe (`200 OK`) |
| `GET` | `/metrics` | Prometheus metrics |
| `POST` | `/ingest/{mapping_id}` | Changefeed webhook sink target |

See [HTTP endpoints](./runner/endpoints.md) for request/response details.

## See also

- [Source setup](./source-setup/cockroachdb-setup.md) — CockroachDB changefeeds that target `/ingest/{mapping_id}`
- [Destination grants](./destination-setup/postgresql-grants.md) — PostgreSQL permissions the runner needs
- [TLS reference](./tls-reference.md) — TLS configuration for the webhook listener and destination connections
- [Troubleshooting](./troubleshooting.md) — Common runner errors and fixes