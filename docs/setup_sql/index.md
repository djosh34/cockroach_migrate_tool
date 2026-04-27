# Setup SQL Guides

Prepare the source CockroachDB SQL and destination PostgreSQL grants before starting `runner`. These guides turn the live bootstrap contract into operator-facing docs, so you do not need to reverse-engineer test helpers or runtime code to know what to run.

## Table Of Contents

- [CockroachDB source setup](./cockroachdb-source-setup.md)
- [PostgreSQL destination grants](./postgresql-destination-grants.md)

## Quick Reference

| Guide | Run on | Run frequency | Why it exists |
| --- | --- | --- | --- |
| [`cockroachdb-source-setup.md`](./cockroachdb-source-setup.md) | The source CockroachDB cluster | Enable rangefeeds once per cluster, capture one cursor per source database, create one changefeed per mapping | The runtime only receives webhook batches after CockroachDB is already configured to emit them. |
| [`postgresql-destination-grants.md`](./postgresql-destination-grants.md) | Each destination PostgreSQL server and database | Once per destination database, then once per destination schema and mapped table | The runtime needs permission to connect, create its helper schema, and read or write the mapped destination tables. |

## Which SQL Runs Where

| Statement family | Database engine | Typical executor | Timing |
| --- | --- | --- | --- |
| `SET CLUSTER SETTING kv.rangefeed.enabled = true;` | CockroachDB | Cluster operator | Before any changefeed creation |
| `USE <database>; SELECT cluster_logical_timestamp() ...;` | CockroachDB | Cluster operator | Once per source database, immediately before creating that database's changefeeds |
| `CREATE CHANGEFEED ... INTO 'webhook-https://.../ingest/<mapping_id>?ca_cert=...'` | CockroachDB | Cluster operator | Once per mapping |
| `GRANT CONNECT, CREATE ON DATABASE ...` | PostgreSQL | Database owner or superuser | Once per destination database and runtime role |
| `GRANT USAGE ON SCHEMA ...` | PostgreSQL | Schema owner or superuser | Once per mapped schema and runtime role |
| `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE ...` | PostgreSQL | Table owner or superuser | Once per mapped table and runtime role |

## Contract Notes

- The source webhook target path is always `/ingest/<mapping_id>`. That path shape is owned by `ingest-contract`.
- The runtime creates `_cockroach_migration_tool`, `stream_state`, and `table_sync_state` by itself after the PostgreSQL grants are in place.
- These docs are the canonical human-readable contract for Task 03 script generation. If you render SQL automatically, the generated output should still match the statements in these guides exactly.
