# CockroachDB to PostgreSQL Migration Tool

Continuously migrates data from CockroachDB to PostgreSQL using changefeed webhooks, with built-in row-level data verification.

The **runner** receives changefeed batches and writes row mutations into PostgreSQL destination tables. The **verify-service** compares source and destination data to confirm migration correctness.

## Documentation

See the **[Operator Guide](docs/operator-guide/index.md)** for installation, configuration, TLS setup, database preparation, architecture, and troubleshooting.

## Development

Use the repository flake for local development:

```bash
nix run .#check
nix run .#lint
nix run .#test
nix develop
```
