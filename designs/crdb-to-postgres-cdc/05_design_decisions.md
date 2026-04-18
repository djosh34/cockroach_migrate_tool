# Design Decisions

## Closed Decisions

- Migration state lives inside each destination database, not in a separate control database.
- Helper state lives in `_cockroach_migration_tool`.
- For each migrated real table, there is a corresponding helper shadow table.
- Webhook `200` means durable persistence into PostgreSQL migration state.
- Real constrained tables are updated by a separate continuous reconcile loop.
- Cutover uses an API-level write freeze.
- MOLT verify checks the real target tables, not the helper shadow tables.
- Minimal primary-key indexing on helper shadow tables is allowed, but must be automatic rather than operator-managed.
- Reconcile runs continuously, not only on demand.
- Deletes are propagated by PostgreSQL SQL during periodic refresh from helper shadow tables into real tables.

## Invariants

- End-to-end tests must be real end-to-end.
- No fake migrations.
- No shortcuts.
- No hidden extra source-side commands after CDC setup is done.
- TLS must be real on the webhook path.
- The same destination container that exposes the webhook endpoint must also manage PostgreSQL-side apply.
- The destination container uses only a scoped PostgreSQL role.

## Remaining Intent

The design package is now aligned to a single chosen design.

Any future changes should refine this design, not reopen competing architecture branches.
