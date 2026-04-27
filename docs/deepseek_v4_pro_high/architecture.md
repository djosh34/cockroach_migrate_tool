# Architecture

The repository now has two shipped runtime surfaces:

- `runner`: the long-running webhook receiver and reconcile engine
- `verify`: the on-demand verification API

Operators are responsible for preparing CockroachDB changefeeds and destination PostgreSQL grants with their own SQL workflow before starting `runner`.

## Data Flow

1. CockroachDB changefeeds deliver row batches and resolved watermarks to `POST /ingest/{mapping_id}` on `runner`.
2. `runner` stores those events in helper tables under `_cockroach_migration_tool`.
3. A reconcile loop copies helper-table state into the real destination tables in PostgreSQL.
4. `verify` compares source and destination tables on demand and reports mismatches.

## Runner

`runner` has two commands:

- `runner validate-config --config <path> [--deep]`
- `runner run --config <path>`

At startup it validates config, bootstraps helper tables in PostgreSQL, then serves the webhook API and reconcile loop.

## Verify

`verify` has two commands:

- `verify validate-config --config <path>`
- `verify run --config <path>`

It exposes an HTTPS API for starting and polling verification jobs. Only one job runs at a time.

## Shared Contract

`ingest-contract` owns the `/ingest/{mapping_id}` route shape shared by CockroachDB changefeeds and the `runner` HTTP surface.
