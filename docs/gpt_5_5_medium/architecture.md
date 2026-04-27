# Architecture

This repository now documents only the shipped runtime surfaces:

- `runner`
- `verify`

Source bootstrap SQL and destination grant SQL are operator-managed concerns outside the shipped binaries.

## Runner Responsibilities

- accept CockroachDB webhook payloads at `/ingest/{mapping_id}`
- persist helper-table state in PostgreSQL
- reconcile helper-table state into destination tables
- expose health and metrics endpoints

## Verify Responsibilities

- validate config
- run one verification job at a time
- expose job lifecycle over HTTP(S)
