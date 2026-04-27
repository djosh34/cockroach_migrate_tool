## Task: Create bash scripts that turn a YAML config file into SQL output files for CockroachDB and PostgreSQL separately <status>completed</status> <passes>true</passes>

<description>
**Goal:** Create two standalone bash scripts that read a simple YAML input file and produce SQL output files — one script for CockroachDB source setup SQL, one script for PostgreSQL destination grants SQL. The scripts must be clearly separated, well-documented, and produce SQL that matches what the docs (Task 02) describe.

The higher-order goal is to give operators a lightweight, auditable alternative to the old Rust binary. Anyone can read the bash and the generated SQL to understand exactly what will be executed.

In scope:
- Create `./scripts/generate-cockroach-setup-sql.sh` that:
  - Reads the CockroachDB YAML config (format defined below)
  - Generates one `.sql` file per source database (e.g. `cockroach-demo_a-setup.sql`, `cockroach-demo_b-setup.sql`)
  - Also generates a combined `cockroach-all-setup.sql` with all database SQL blocks concatenated
  - Uses `envsubst` for simple variable substitution and `yq` (or `python3 -c` / `ruby -r yaml -r json -e`) for YAML parsing — must declare its dependencies clearly in a comment header
  - Must handle multi-mapping scenarios (multiple mapping IDs targeting the same database get merged into one SQL file)
  - Output SQL is valid and matches the contract in `./docs/setup_sql/cockroachdb-source-setup.md`
- Create `./scripts/generate-postgres-grants-sql.sh` that:
  - Reads the PostgreSQL grants YAML config (format defined below)
  - Generates one `.sql` file per destination database (e.g. `postgres-app_a-grants.sql`, `postgres-app_b-grants.sql`)
  - Also generates a combined `postgres-all-grants.sql` with all database SQL blocks concatenated
  - Uses the same dependency strategy as the CockroachDB script
  - Deduplicates GRANT statements when multiple mappings reference the same table+role
  - Output SQL is valid and matches the contract in `./docs/setup_sql/postgresql-destination-grants.md`
- Both scripts must include a `--help` flag, a `--dry-run` flag that prints what would be generated, and exit with clear error messages on invalid input
- Output SQL files go to an `./output/` directory by default, configurable via `--output-dir`
- Create `./scripts/README.md` documenting both scripts, their dependencies, and usage examples

**Input YAML format for CockroachDB source setup:**

```yaml
# cockroach-setup-config.yml — input to generate-cockroach-setup-sql.sh
cockroach:
  url: "postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require"
webhook:
  base_url: "https://runner.example.internal:8443"
  ca_cert_path: "ca.crt"          # path to CA cert file; script reads and base64-encodes it
  resolved: "5s"
mappings:
  - id: "app-a"
    source:
      database: "demo_a"
      tables:
        - "public.customers"
        - "public.orders"
  - id: "app-b"
    source:
      database: "demo_b"
      tables:
        - "public.invoices"
```

**Input YAML format for PostgreSQL destination grants:**

```yaml
# postgres-grants-config.yml — input to generate-postgres-grants-sql.sh
mappings:
  - id: "app-a"
    destination:
      database: "app_a"
      runtime_role: "migration_user_a"
      tables:
        - "public.customers"
        - "public.orders"
  - id: "app-b"
    destination:
      database: "app_b"
      runtime_role: "migration_user_b"
      tables:
        - "public.invoices"
```

**Expected SQL output per database (CockroachDB):**
```sql
-- Source bootstrap SQL
-- Cockroach URL: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require
-- Apply each statement with a Cockroach SQL client against the source cluster.
-- Capture the cursor once, then replace __CHANGEFEED_CURSOR__ in the CREATE CHANGEFEED statements below.

SET CLUSTER SETTING kv.rangefeed.enabled = true;
SELECT cluster_logical_timestamp() AS changefeed_cursor;

-- Source database: demo_a

-- Mapping: app-a
-- Selected tables: public.customers, public.orders
-- Replace __CHANGEFEED_CURSOR__ below with the decimal cursor returned above before running the CREATE CHANGEFEED statement.
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders INTO 'webhook-https://runner.example.internal:8443/v1/ingest/app-a?ca_cert=<base64encoded>' WITH cursor = '__CHANGEFEED_CURSOR__', initial_scan = 'yes', envelope = 'enriched', enriched_properties = 'source', resolved = '5s';
```

**Expected SQL output per database (PostgreSQL):**
```sql
-- PostgreSQL grants SQL
-- Destination database: app_a
-- Helper schema: _cockroach_migration_tool

GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;
GRANT USAGE ON SCHEMA public TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.orders TO migration_user_a;
```

Out of scope:
- Any Rust code or binary compilation
- Docker images or containerization of the scripts
- Applying the SQL automatically (the scripts only generate .sql files)
- Integrations with CI/CD pipelines (the scripts are run manually or in a pipeline by the operator)

Decisions already made:
- PostgreSQL generation and CockroachDB generation are kept in clearly separate scripts — no combined mode
- Input format is YAML to match the existing config shape operators already understand
- Output is `.sql` files, one per database plus a combined file
- `envsubst` for variable substitution in SQL templates (lightweight, no heavy template engine)
- `yq` (or python3 fallback) for YAML parsing
- `base64` CLI for CA cert encoding
- Scripts are pure bash with no compiled dependencies
</description>

<acceptance_criteria>
- [x] `./scripts/generate-cockroach-setup-sql.sh` exists, is executable, and:
  - [x] Accepts a YAML file matching the documented CockroachDB config format
  - [x] Generates per-database `.sql` files and a combined `cockroach-all-setup.sql`
  - [x] Produces valid SQL matching the contract in `./docs/setup_sql/cockroachdb-source-setup.md`
  - [x] Has `--help`, `--dry-run`, `--output-dir` flags
  - [x] Handles multi-mapping merging (same database from multiple mapping IDs)
  - [x] Validates required YAML keys and exits with clear error if missing
- [x] `./scripts/generate-postgres-grants-sql.sh` exists, is executable, and:
  - [x] Accepts a YAML file matching the documented PostgreSQL grants config format
  - [x] Generates per-database `.sql` files and a combined `postgres-all-grants.sql`
  - [x] Produces valid SQL matching the contract in `./docs/setup_sql/postgresql-destination-grants.md`
  - [x] Has `--help`, `--dry-run`, `--output-dir` flags
  - [x] Deduplicates identical GRANT statements
  - [x] Validates required YAML keys and exits with clear error if missing
- [x] `./scripts/README.md` documents dependencies, usage, and examples
- [x] Manual verification: run both scripts against the example YAMLs above and confirm the output SQL matches the expected output exactly
- [x] Manual verification: run both scripts with `--dry-run` and confirm no files are written
- [x] Manual verification: run both scripts with missing/invalid YAML keys and confirm they exit non-zero with a readable error
- [x] `make check` — passes cleanly (scripts-only, no Rust code impacted)
- [x] `make lint` — passes cleanly (if shellcheck is in lint pipeline, scripts must pass shellcheck)
</acceptance_criteria>

<plan>.ralph/tasks/story-28-replace-setup-sql-with-docs-and-bash-scripts/03-task-create-bash-scripts-to-generate-sql-from-yaml_plans/2026-04-28-generate-setup-sql-scripts-plan.md</plan>
