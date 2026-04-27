# Getting Started

This guide covers the supported runtime surfaces only: `runner` and `verify`.

## 1. Prepare Source And Destination SQL

Before starting `runner`, create the CockroachDB changefeeds and apply the destination PostgreSQL grants with operator-managed SQL for your environment.

## 2. Write `runner.yml`

Define:

- `webhook.bind_addr`
- `webhook.mode` and optional TLS files
- `reconcile.interval_secs`
- one or more `mappings`

Each mapping names a CockroachDB source database plus selected tables and a PostgreSQL destination connection.

## 3. Start `runner`

Validate first:

```bash
runner validate-config --config ./runner.yml
```

Then run it:

```bash
runner run --config ./runner.yml
```

`runner` expects source changefeeds to already post into `/ingest/{mapping_id}`.

## 4. Write `verify-service.yml`

Configure:

- `listener.bind_addr`
- listener TLS certificates
- source database URL and optional TLS material
- destination database URL and optional TLS material

## 5. Start `verify`

Validate first:

```bash
verify validate-config --config ./verify-service.yml
```

Then run the API:

```bash
verify run --config ./verify-service.yml
```
