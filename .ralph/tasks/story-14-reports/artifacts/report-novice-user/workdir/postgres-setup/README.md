# PostgreSQL Setup Artifacts

These artifacts keep PostgreSQL grants explicit and manual. Render them, review them, run each `grants.sql`, then start the runner.

Once the grants exist, `runner run --config <path>` bootstraps helper objects inside schema `_cockroach_migration_tool` automatically, but it does not create roles or execute grants for you.

No superuser requirement is assumed or recommended.

## Mappings

- `app-a`: database `app_a` role `migration_user_a`
  Artifacts: `app-a/grants.sql`, `app-a/README.md`
