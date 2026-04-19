# PostgreSQL Setup For `app-a`

Run `grants.sql` while connected to database `app_a` before starting the runner.

These grants stay manual and explicit by design. The runtime role is `migration_user_a`.

No superuser requirement is assumed or recommended.

After the grants exist, `runner run --config <path>` creates helper objects in schema `_cockroach_migration_tool` automatically.
If `_cockroach_migration_tool` already exists, it must already be owned by `migration_user_a`.
