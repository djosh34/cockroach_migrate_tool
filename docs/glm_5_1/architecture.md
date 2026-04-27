# Architecture

The supported product split is now:

- `runner` for webhook ingestion and reconciliation
- `verify` for on-demand correctness checks

Operators must provision CockroachDB changefeeds and destination grants outside this repository's shipped binaries.

## Core Runtime

`runner` receives changefeed webhooks, persists them into helper tables, and reconciles helper state into destination PostgreSQL tables.

## Verification

`verify` compares source and destination tables and returns structured job results over HTTP(S).

## Shared Route Contract

`ingest-contract` defines `/ingest/{mapping_id}` so CockroachDB sinks and `runner` stay aligned.
