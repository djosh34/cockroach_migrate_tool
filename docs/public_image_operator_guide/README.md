# Operator Guide — Published Images

This guide covers deploying the CockroachDB-to-PostgreSQL migration tool from published container images — from pulling images to verifying data integrity after migration.

## Images

| Image | Purpose | Primary registry |
|-------|---------|-------------------|
| `runner-image` | Receives CockroachDB changefeed webhooks and writes rows into PostgreSQL destinations | GHCR |
| `verify-image` | Compares source and destination data to confirm migration correctness | GHCR |

See **[Image References](image-references.md)** for exact pull commands and tag conventions.

## Operator workflow

```
1.  Pull images                →  image-references.md
2.  Prepare source CockroachDB →  source-setup.md → source-setup/cockroachdb-setup.md
3.  Grant destination perms    →  destination-grants.md → destination-setup/postgresql-grants.md
4.  Configure and run runner   →  runner.md → runner/getting-started.md
5.  Configure and run verify   →  verify-service.md → verify/getting-started.md
6.  Run a verify job           →  verify/job-lifecycle.md
7.  Troubleshoot problems      →  troubleshooting.md
```

## All pages

| Document | What it covers |
|----------|---------------|
| [Image References](image-references.md) | GHCR pull commands, tag format, Quay mirror |
| [Source Setup](source-setup.md) | CockroachDB setup overview |
| [CockroachDB setup](source-setup/cockroachdb-setup.md) | Full SQL walkthrough, sink URL encoding, checklist |
| [Destination Grants](destination-grants.md) | PostgreSQL grants overview |
| [PostgreSQL grants](destination-setup/postgresql-grants.md) | Per-database SQL, worked examples, checklist |
| [Runner](runner.md) | Runner overview, sub-page index |
| [Runner getting started](runner/getting-started.md) | Pull, configure, validate, and run |
| [Runner configuration](runner/configuration.md) | Full YAML reference |
| [Runner endpoints](runner/endpoints.md) | `/healthz`, `/metrics`, `/ingest/{mapping_id}` |
| [Verify Service](verify-service.md) | Verify-service overview, sub-page index |
| [Verify getting started](verify/getting-started.md) | Pull, configure, validate, and run |
| [Verify configuration](verify/configuration.md) | Full YAML reference |
| [Verify job lifecycle](verify/job-lifecycle.md) | Start, poll, and stop verify jobs |
| [TLS reference](tls-reference.md) | TLS configuration for all components |
| [Troubleshooting](troubleshooting.md) | Common setup failures and diagnostics |