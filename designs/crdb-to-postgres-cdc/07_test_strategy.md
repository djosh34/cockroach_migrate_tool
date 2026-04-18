# Test Strategy

## Goal

The final implementation must prove correctness under:

- initial scan
- live updates
- deletes
- restart after partial failure
- duplicate webhook delivery
- active PK and FK constraints
- multiple database mappings
- limited PostgreSQL permissions

## Test Layers

### 1. Unit Tests

- payload parsing
- table routing from `source`
- PK extraction for single-column and composite keys
- row normalization
- duplicate event collapse logic
- resolved checkpoint advancement rules
- schema normalization and comparison logic

### 2. Integration Tests With Real Databases

These should use real CockroachDB and PostgreSQL containers.

Core integration scenarios:

- `initial_scan = 'yes'` on populated source tables
- live inserts, updates, and deletes after feed start
- receiver returns `500` on first attempt and succeeds on retry
- receiver crash after helper-table persistence but before reconcile
- changefeed cancel and resume from stored resolved timestamp
- MOLT verify pass case
- MOLT verify mismatch case
- API-level write freeze followed by drain-to-zero cutover
- one destination container managing multiple source-database to destination-database mappings

### 3. Schema-Shape Matrix

Minimum matrix:

- parent-child-grandchild tables
- composite primary key table
- many-to-many join table
- unique constraints separate from PK
- `ON DELETE CASCADE`
- nullable FK column

Recommended extra matrix:

- self-referential tree table
- wide table with mixed scalar types
- tables excluded from migration

## Apply-Strategy-Specific Tests

### Helper Shadow Tables

- raw batch persisted into helper shadow tables before HTTP `200`
- duplicate webhook batch does not create incorrect net state
- malformed payload is rejected and recorded
- row batches correctly update the helper shadow tables
- resolved messages correctly advance helper tracking state

### Reconcile Worker

- reconcile only advances through a resolved checkpoint
- upsert ordering respects FK dependencies
- delete ordering respects FK dependencies
- repeated reconcile passes are idempotent
- once helper shadow tables are correct, real tables converge to the same state

## End-To-End Integrity Rules

These rules are mandatory for the end-to-end suite:

- no fake migrations
- no shortcuts that bypass the real webhook and reconcile path
- no hidden helper logic inside the binary that exists only for tests
- no extra shell commands, SQL commands, or manual scripts against the source after CDC setup is complete
- MOLT verify must inspect the real destination tables, not the helper shadow tables
- HTTP must run with TLS
- the destination container must use only its scoped PostgreSQL role, not superuser privileges
- the same container that exposes the webhook endpoint must also manage the PostgreSQL-side apply flow

## Restartability Tests

- restart receiver before acknowledging batch
- restart receiver after acknowledging helper-table persistence but before reconcile
- restart reconcile worker mid-run
- recreate changefeed from last reconciled resolved timestamp
- recreate changefeed from an older resolved timestamp and confirm idempotent replay is safe

## Permission Tests

- scoped PostgreSQL role can:
  - create helper tables
  - use `COPY FROM STDIN`
  - upsert target rows
  - delete target rows
- scoped CockroachDB role can:
  - create changefeed on granted tables

## Novice User Acceptance

At least one end-to-end scenario must prove the novice-user path:

- README alone is sufficient
- one copyable starting config example works as written
- container build works directly through `docker build` or `docker compose up`
- no wrapper bash scripts are required for the user path
- any step that requires “look up how this works” is treated as a failure

## Verification Tests

- wrapper fails when MOLT reports mismatches even if process exit code is `0`
- wrapper passes only when all per-table mismatch counters are `0`
- final cutover test blocks source writes, waits for zero lag, runs verification, and only then marks cutover as ready

## Performance Tests

These can come after the correctness suite is green.

Measure:

- raw ingest throughput
- merge throughput
- checkpoint lag growth under write load
- effect of batch size on final merge latency
- effect of `COPY`-based staging for large initial snapshots
- behavior under high source write churn during migration

## Test Philosophy

- never skip a failing real-db test
- do not weaken constraints to make tests pass
- a flaky restart/retry test is a real bug, not a nuisance
