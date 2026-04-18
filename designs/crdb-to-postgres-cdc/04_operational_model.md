# Operational Model

## One Destination Container

The destination side is one container running one binary that:

- exposes the HTTPS webhook endpoint
- connects to PostgreSQL with a scoped role only
- manages helper shadow tables and tracking tables
- runs the continuous reconcile loop
- manages multiple source-database to destination-database mappings from one config

No superuser PostgreSQL role is assumed.

## Source Setup

The source-side setup command should:

- capture `cluster_logical_timestamp()`
- create one changefeed per source database with:
  - explicit `cursor`
  - `initial_scan = 'yes'`
  - `envelope = 'enriched'`
  - `enriched_properties = 'source'`
  - `resolved`
- print or persist:
  - source database
  - job id
  - stream id
  - starting cursor
  - selected tables

After CDC setup is completed, the intended operating model is:

- no further raw commands against the source database are needed for the migration flow
- the destination container keeps the migration moving on its own

## Cutover

The selected cutover model is:

- keep PostgreSQL continuously shadowing CockroachDB
- run MOLT verify against the real target tables during that period
- at handover time:
  - block writes at the API boundary
  - wait for CDC to drain
  - let the reconcile loop finish
  - require MOLT verify to report equality
  - switch traffic to PostgreSQL

This is the chosen write-freeze model.

## Library Direction

Implementation should use established libraries instead of reinventing obvious infrastructure.

Direction already chosen:

- `sqlx` for PostgreSQL access
- `thiserror` for application error types
- an established Rust HTTP framework with HTTPS support
- `serde` / `serde_yaml` for config
- `clap` for CLI

This is a design rule. Tasks should not exist merely to install libraries; a library is introduced only in the story where it is actually needed.

## Novice User Constraint

The README must be sufficient for a novice user.

That means:

- the user should not need to inspect source code
- the user should not need to infer behavior from scripts
- quick start must be short and direct
- config examples must be copyable and working
- container build and run must work directly through normal Docker commands
- any need to "go look up how this works" is a failure of the delivered UX
