# Shadow Table Architecture

## Selected Design

Only one design is selected:

- one CockroachDB webhook changefeed per source database
- one destination container and one destination binary
- one helper schema per destination database: `_cockroach_migration_tool`
- one helper shadow table per migrated real table
- webhook `200` only after durable persistence into helper migration state
- continuous reconcile from helper shadow tables into the real constrained tables

No alternative design is kept in this design package.

## Core Shape

For each migrated destination database:

- create `_cockroach_migration_tool`
- create tracking tables
- create one helper shadow table for each migrated real table

Each helper shadow table mirrors the real table's data columns and is optimized for ingest rather than serving.

Shadow table rules:

- no foreign keys
- no secondary indexes
- no serving-oriented uniqueness constraints
- a minimal primary-key index is allowed when the runner decides it is needed
- that minimal primary-key index must be automatic, not operator-managed

The real destination tables remain the real target:

- primary keys stay enabled
- foreign keys stay enabled
- serving indexes stay enabled

## Webhook Ingest Rule

The destination receiver accepts webhook POSTs over HTTPS.

For row batches:

- detect the target table from `source.table_name`
- translate the row payload into PostgreSQL column values
- apply the row to the corresponding helper shadow table
- update stream tracking state
- commit
- return `200`

For resolved messages:

- update the latest received resolved watermark
- commit
- return `200`

If persistence into helper migration state fails:

- return non-`200`
- let CockroachDB retry

## Reconcile Rule

The real target tables are updated by a separate reconcile loop running continuously inside the same destination container.

Reconcile rules:

1. Upsert passes run in dependency order.
2. Delete passes run in reverse dependency order.
3. The process must be repeatable many times.
4. Reconcile advances state only after a successful full pass.

Recommended SQL shape:

- upserts:
  - `INSERT INTO real_table (...) SELECT ... FROM shadow_table ON CONFLICT (...) DO UPDATE ...`
- deletes:
  - `DELETE FROM real_table r WHERE NOT EXISTS (SELECT 1 FROM shadow_table s WHERE pk matches)`

Delete handling is intentionally simple:

- if a row is absent from the helper shadow table during refresh, PostgreSQL deletes it from the real table through SQL in the periodic reconcile pass

## Continuous Operation

The system is not designed only for one week of shadowing.

It must be able to:

- keep running continuously
- keep helper shadow tables current
- keep real tables repeatedly refreshed from helper shadow tables
- remain ready for cutover whenever the operator chooses
