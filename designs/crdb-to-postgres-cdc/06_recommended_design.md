# Recommended Design

## Recommendation

Use one CockroachDB webhook changefeed per source database and land it into per-table helper shadow tables inside `_cockroach_migration_tool` in the destination database. Then run a separate reconcile loop from the helper shadow tables into the real constrained tables.

This is the recommended design because it is the simplest one that still survives the real findings from the investigation.

## Core Shape

For each migrated destination database:

- create `_cockroach_migration_tool`
- create a small set of tracking tables
- for each real migrated table, create one matching helper shadow table in `_cockroach_migration_tool`

The helper shadow table should mirror the real table's data columns, but strip out the heavy structure that slows ingest:

- no foreign keys
- no secondary indexes
- no serving-oriented constraints

The real destination tables stay real:

- primary keys stay
- foreign keys stay
- normal indexes stay

## Webhook Success Rule

The receiver returns `200` after the incoming webhook message has been durably committed into PostgreSQL migration state.

In practice that means:

- row batches:
  - applied to the relevant `_cockroach_migration_tool.<table>` shadow tables
  - progress metadata updated
  - transaction committed
  - then `200`
- resolved messages:
  - stored in the migration tracking tables
  - transaction committed
  - then `200`

The receiver does not wait for the real constrained tables to be updated before returning `200`.

## Runtime Shape

The destination side should be one container running one binary that:

- exposes the HTTPS webhook endpoint
- connects to PostgreSQL with a scoped role
- manages helper shadow tables and tracking tables
- runs the periodic reconcile loop
- manages multiple source-database to destination-database mappings from one config

It should be able to keep running continuously until cutover, not only for a fixed shadowing window.

## Minimal Tracking Tables

Keep this small.

Recommended minimum:

- `_cockroach_migration_tool.stream_state`
  - one row per source-to-destination stream
  - stores:
    - source database
    - source job id
    - starting cursor
    - latest received resolved timestamp
    - latest successfully reconciled resolved timestamp
    - stream status
- `_cockroach_migration_tool.table_sync_state`
  - one row per migrated table
  - stores:
    - last successful sync time
    - last successful sync watermark
    - last error

That is enough for a first version.

## Source Setup

The source-side command should:

- capture `cluster_logical_timestamp()`
- create the changefeed with:
  - `initial_scan = 'yes'`
  - explicit `cursor`
  - `envelope = 'enriched'`
  - `enriched_properties = 'source'`
  - `resolved`
- print or persist:
  - source database
  - stream id
  - starting cursor
  - job id
  - selected tables

This command must be rerunnable.

After CDC setup is done, the intended production model is that the destination container keeps the migration moving without requiring more raw source-side commands.

## Destination Receiver

The destination receiver should stay dumb.

For each incoming row event:

- detect the target table from `source.table_name`
- translate Cockroach row shape to PostgreSQL column values
- apply the change to the corresponding helper shadow table
- do not touch the real constrained table inside the webhook handler

For each resolved message:

- update `stream_state.latest_received_resolved`

If any part of that transaction fails:

- return non-`200`
- let Cockroach retry

## Reconcile Loop

Run a separate reconcile worker in the destination container.

Its job is to copy helper shadow state into the real tables until the real tables match the helper shadow tables.

Recommended simple order:

1. Upsert passes in dependency order:
   - parents before children
2. Delete passes in reverse dependency order:
   - children before parents

Recommended SQL shape:

- upserts:
  - `INSERT INTO real_table (...) SELECT ... FROM shadow_table ON CONFLICT (...) DO UPDATE ...`
- deletes:
  - `DELETE FROM real_table r WHERE NOT EXISTS (SELECT 1 FROM shadow_table s WHERE pk matches)`

After a full successful table-order pass:

- advance `latest successfully reconciled resolved timestamp`

## Cutover

The intended operational flow is:

- keep PostgreSQL shadowing CockroachDB continuously until handover time
- run MOLT verify repeatedly during that period
- when handover time comes:
  - block incoming writes at the API layer
  - let CDC drain
  - let the reconcile loop finish
  - require MOLT verify to report equality
  - switch traffic to PostgreSQL

That is the write-freeze cutover model.

## Why This Design Is Preferred

- It matches the desired helper-schema layout.
- It is simpler than a generic event-log merge engine.
- It respects active constraints on the real tables.
- It gives a clean webhook acknowledgement rule.
- It is easy to inspect in PostgreSQL during long-running continuous shadowing.

## Library Direction

The implementation should use established libraries where they materially reduce risk and boilerplate.

Current direction:

- `sqlx` for PostgreSQL access
- `thiserror` for application error types
- an established async Rust HTTP framework with TLS support, such as `axum` plus standard TLS crates, rather than hand-rolled HTTP handling
- `serde` / `serde_yaml` for configuration
- `clap` for CLI handling

This is design guidance only. Task breakdown should introduce each library only in the story where it is actually needed.

## Novice User Constraint

The final README and quick start must be sufficient for a novice user.

That means:

- no requirement to inspect source code or repository internals
- no requirement to reverse engineer command sequences from scripts
- copyable config examples
- few steps
- direct commands that work as written
- container build and startup that work without wrapper bash scripts

## Known Tradeoff

The simplest delete handling is a periodic anti-join pass from real tables against helper shadow tables.

That is deliberately simple, but it may become expensive on very large tables.

If that becomes a real performance problem later, the next step is not to redesign everything. The next step is to add a more incremental delete-tracking optimization on top of this design.
