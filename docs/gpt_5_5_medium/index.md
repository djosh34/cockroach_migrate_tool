# CockroachDB Migration Tool Documentation

This documentation describes the project from the implementation in this repository. It covers how to run the tool, how to build or install it, and how the migration system is structured internally.

## Documents

- [Getting Started](getting-started.md): quick Docker path, minimal configs, common commands, and examples.
- [Installation](installation.md): source checkout options, local builds, Docker image builds, and validation commands.
- [Architecture](architecture.md): migration goal, runtime flow, component responsibilities, data model, APIs, and operational behavior.

## What This Project Does

The project migrates selected CockroachDB tables into PostgreSQL-compatible destination databases using three operator-facing pieces:

1. `setup-sql` emits one-time SQL for source CockroachDB changefeeds and destination PostgreSQL grants.
2. `runner` receives CockroachDB enriched webhook changefeed events, persists them into destination-side shadow tables, and reconciles those shadow tables into the real destination tables.
3. `verify-service` runs the vendored Molt verifier behind an HTTP API so operators can start, poll, stop, and inspect verification jobs.

The destination-side helper schema is `_cockroach_migration_tool`. The runner creates and maintains its tracking tables and per-mapping shadow tables inside that schema.

