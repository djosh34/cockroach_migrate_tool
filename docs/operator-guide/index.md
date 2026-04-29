# CockroachDB to PostgreSQL Migration Tool — Operator Guide

Two published container images — `runner-image` and `verify-image` — together form a complete CockroachDB-to-PostgreSQL migration pipeline.

## What problem does this solve?

You have data in CockroachDB that must move into PostgreSQL, and the migration cannot be a one-time snapshot. CockroachDB changefeeds emit every row mutation as a real-time stream. This tool receives those streams, writes them into PostgreSQL destination tables, and lets you verify that every row arrived correctly.

## How the pieces fit together

```
CockroachDB source
      │
      │  changefeeds push row batches
      ▼
┌──────────┐     ┌────────────────┐
│  runner   │────▶│  PostgreSQL    │
│  image    │     │  destination   │
└──────────┘     └────────────────┘
                      │
                      │  verify-service
                      │  reads both sides
                      ▼
               ┌────────────────┐
               │  verify-image   │
               │  checks every   │
               │  row matches    │
               └────────────────┘
```

| Image | Role | Registry |
|-------|------|----------|
| `runner-image` | Receives changefeed webhooks, writes rows into PostgreSQL | GHCR (primary), Quay (mirror) |
| `verify-image` | Compares source and destination data row-by-row | GHCR (primary), Quay (mirror) |

Both images are multi-platform (`linux/amd64`, `linux/arm64`) and tagged by full Git commit SHA.

## Operator workflow

1. **[Getting Started](getting-started.md)** — One-page happy-path walkthrough covering the entire flow.
2. **[Install the images](installation.md)** — Pull from GHCR, authenticate, understand tags.
3. **[Set up TLS certificates](tls-configuration.md)** — Certificates are required before writing component configs. Every TLS field across all components in one place.
4. **[Review the configuration reference](config-reference.md)** — Single-page hub for all config files, common operator decisions, and where to find field-level details. Read this before writing runner or verify-service configs.
5. **[Configure CockroachDB and PostgreSQL](setup-sql.md)** — Enable rangefeeds, create changefeeds, grant destination permissions.
6. **[Configure and run the runner](runner.md)** — Write YAML config, validate, start the webhook listener.
7. **[Configure and run the verify-service](verify-service.md)** — Write YAML config, validate, start the API, run and poll verify jobs.
8. **[Understand the architecture](architecture.md)** — Internals: webhook ingestion, reconciliation, helper schema, table comparison.
9. **[Troubleshoot](troubleshooting.md)** — Diagnose common failures.

**Order matters:** TLS certificates must exist before writing runner or verify-service configs (configs reference certificate paths). CockroachDB changefeeds and PostgreSQL grants must be in place before the runner starts. CockroachDB retries webhook deliveries, so changefeeds can be created before the runner is reachable — but no data flows until the runner is listening.

## Pages

| Page | Covers |
|------|--------|
| [Getting Started](getting-started.md) | Complete end-to-end walkthrough in one page |
| [Installation](installation.md) | Pull commands, tags, GHCR/Quay, authentication, running containers, log format |
| [TLS Configuration](tls-configuration.md) | TLS settings for runner listener, runner destinations, verify listener, verify database connections |
| [Configuration Reference](config-reference.md) | Single-page hub for all config files: runner, verify-service, TLS, common operator decisions, and where to find field-level details |
| [Source & Destination Setup](setup-sql.md) | CockroachDB changefeeds, PostgreSQL grants, SQL generator scripts |
| [Runner](runner.md) | CLI, configuration reference, HTTP endpoints, webhook payload format, Docker Compose |
| [Verify-Service](verify-service.md) | CLI, configuration reference, job lifecycle API, Docker Compose |
| [Architecture](architecture.md) | Webhook ingestion, reconciliation loop, `_cockroach_migration_tool` helper schema, table comparison internals |
| [Troubleshooting](troubleshooting.md) | Common failures and diagnostic steps |
