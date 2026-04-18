# CRDB To PostgreSQL CDC Migration Design

## Purpose

This design package captures the investigation, selected architecture, tradeoffs,
and recommended direction for a one-time CockroachDB to PostgreSQL
migration system that:

- can perform initial load plus live catch-up via CockroachDB CDC
- applies into PostgreSQL without requiring superuser access
- can be resumed after partial failure without retransferring all rows
- supports multiple source databases and multiple destination databases
- can verify end-state correctness with MOLT verify

## User Intent Captured

The current user intent for this design package is:

- write all investigation and design work directly into Markdown files under this directory
- investigate on real CockroachDB and PostgreSQL instances using the repo's existing harnesses
- evaluate multiple PostgreSQL apply strategies, not just one
- keep FK and PK constraints enabled in the target during the migration
- design for one-time migration, not long-lived bidirectional replication
- design for a source-side setup command that can run in a pipeline and can be rerun
- design for a destination-side transfer binary living in a container on the PostgreSQL host
- reuse MOLT verify after migration to validate correctness
- treat schema generation as out of scope, but include schema comparison / validation
- keep the destination runner able to operate continuously, not only for a one-week shadowing window
- keep operator experience simple enough that a novice can succeed from the README alone

## Current Status

This directory is being updated as the investigation proceeds. It is not final until
the comparison report and recommendation sections are complete and the grill-me
question round has been incorporated.
