# Getting Started

Prepare your SQL first, then start the two supported services.

## Runner

```bash
runner validate-config --config ./runner.yml
runner run --config ./runner.yml
```

## Verify

```bash
verify validate-config --config ./verify-service.yml
verify run --config ./verify-service.yml
```

The source CockroachDB changefeeds must already point at the `runner` ingest endpoints.
