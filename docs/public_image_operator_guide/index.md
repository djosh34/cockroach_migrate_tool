# Operator Guide: Published Images

Deploy and run the CockroachDB-to-PostgreSQL migration tool from its published Docker images. No other repository documentation is required.

## Images

| Image | Purpose | Registry |
|-------|---------|----------|
| `runner-image` | Receives CockroachDB changefeed webhooks, writes rows into PostgreSQL | GHCR (primary), Quay (mirror) |
| `verify-image` | Compares source and destination data to confirm migration correctness | GHCR (primary), Quay (mirror) |

See [Image References](image-references.md) for pull commands, tag format, and authentication.

## Operator workflow

```
 ┌─────────────────┐    ┌──────────────────────┐    ┌─────────────────┐
 │ 1. Source setup  │───▶│ 2. Destination grants│───▶│ 3. Run runner    │
 │ (CockroachDB)    │    │ (PostgreSQL)          │    │                  │
 └─────────────────┘    └──────────────────────┘    └────────┬─────────┘
                                                            │
                                              changefeeds  │
                                              deliver data ▼
                                                            │
                                                         ┌──┴───┐
                                                         │ 4. Run│
                                                         │  verify│
                                                         │  job  │
                                                         └──────┘
```

1. **[Prepare source CockroachDB](source-setup/cockroachdb-setup.md)** — Enable rangefeeds, capture cursors, create changefeeds.
2. **[Grant destination PostgreSQL permissions](destination-setup/postgresql-grants.md)** — Give the runtime role access to databases, schemas, and tables.
3. **[Configure and start the runner](runner/getting-started.md)** — Write config, validate, and run.
4. **[Configure and start the verify-service](verify/getting-started.md)** — Write config, validate, and run.
5. **[Run a verify job](verify/job-lifecycle.md)** — Start, poll, and stop verify jobs.

> **Order matters:** Create changefeeds and destination grants before starting the runner. CockroachDB retries webhook deliveries, so changefeeds can be created while the runner is not yet running — but data will not flow until the runner is reachable.

## All pages

| Page | What it covers |
| --- | --- |
| [Image References](image-references.md) | GHCR pull commands, tag format, Quay mirror, running containers |
| [Source Setup](source-setup.md) | CockroachDB rangefeed, cursor, and changefeed setup (overview) |
| [CockroachDB setup](source-setup/cockroachdb-setup.md) | Full walkthrough: SQL, sink URL encoding, checklist |
| [Destination Grants](destination-grants.md) | PostgreSQL permissions overview |
| [PostgreSQL grants](destination-setup/postgresql-grants.md) | Per-database SQL, worked examples, checklist |
| [Runner](runner.md) | Runner overview and sub-page index |
| [Runner getting started](runner/getting-started.md) | Pull, configure, validate, and run the runner |
| [Runner configuration](runner/configuration.md) | Full YAML reference |
| [Runner endpoints](runner/endpoints.md) | `/healthz`, `/metrics`, `/ingest/{mapping_id}` |
| [Verify Service](verify-service.md) | Verify-service overview and sub-page index |
| [Verify getting started](verify/getting-started.md) | Pull, configure, validate, and run the verify-service |
| [Verify configuration](verify/configuration.md) | Full YAML reference |
| [Verify job lifecycle](verify/job-lifecycle.md) | Start, poll, and stop verify jobs |
| [TLS reference](tls-reference.md) | TLS settings for all components |
| [Troubleshooting](troubleshooting.md) | Common failures and fixes |