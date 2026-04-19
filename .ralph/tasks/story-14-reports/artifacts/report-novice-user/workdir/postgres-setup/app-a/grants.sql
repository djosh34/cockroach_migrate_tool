-- PostgreSQL grants for mapping `app-a`
-- Destination database: app_a
-- Runtime role: migration_user_a
-- Helper schema: _cockroach_migration_tool

GRANT CONNECT, TEMPORARY, CREATE ON DATABASE "app_a" TO "migration_user_a";
GRANT USAGE ON SCHEMA public TO "migration_user_a";
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE "public"."customers" TO "migration_user_a";
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE "public"."orders" TO "migration_user_a";
