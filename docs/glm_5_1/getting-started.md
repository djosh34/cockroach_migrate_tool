# Getting Started

1. Prepare CockroachDB changefeeds and destination PostgreSQL grants with your own SQL workflow.
2. Write `runner.yml`.
3. Run `runner validate-config --config ./runner.yml`.
4. Start `runner run --config ./runner.yml`.
5. Write `verify-service.yml`.
6. Run `verify validate-config --config ./verify-service.yml`.
7. Start `verify run --config ./verify-service.yml`.

`runner` only works once the source changefeeds already target `/ingest/{mapping_id}`.
