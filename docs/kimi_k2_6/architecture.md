# Architecture

The current system ships two runtime components:

- `runner`
- `verify`

CockroachDB changefeed creation and PostgreSQL grant setup happen outside the shipped binaries.

`runner` receives changefeed webhooks, lands them in helper tables, and reconciles those helper tables into the destination schema. `verify` compares source and destination data on demand.
