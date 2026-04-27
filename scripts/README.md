# Setup SQL Generators

These scripts turn a small YAML config into auditable SQL files for manual execution.

- `generate-cockroach-setup-sql.sh`
  - renders CockroachDB source setup SQL
  - writes one file per source database plus `cockroach-all-setup.sql`
- `generate-postgres-grants-sql.sh`
  - renders PostgreSQL destination grant SQL
  - writes one file per destination database plus `postgres-all-grants.sql`

## Dependencies

Required for both scripts:

- `bash`
- `envsubst`
- `sort`

Cockroach-only:

- `base64`

YAML parsing:

- preferred: `yq`
- fallback: `python3`

The scripts prefer `yq` when it is present. If `yq` is not installed, they fall back to a built-in `python3` parser that supports the task's documented config shape.

## Usage

```bash
./scripts/generate-cockroach-setup-sql.sh [--dry-run] [--output-dir ./output] ./cockroach-setup-config.yml
./scripts/generate-postgres-grants-sql.sh [--dry-run] [--output-dir ./output] ./postgres-grants-config.yml
```

Common flags:

- `--help`
  - print usage
- `--dry-run`
  - print the files that would be generated without writing them
- `--output-dir`
  - choose a directory other than the default `./output`

## Input Shapes

Cockroach source setup:

```yaml
cockroach:
  url: "postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require"
webhook:
  base_url: "https://runner.example.internal:8443"
  ca_cert_path: "ca.crt"
  resolved: "5s"
mappings:
  - id: "app-a"
    source:
      database: "demo_a"
      tables:
        - "public.customers"
        - "public.orders"
```

PostgreSQL destination grants:

```yaml
mappings:
  - id: "app-a"
    destination:
      database: "app_a"
      runtime_role: "migration_user_a"
      tables:
        - "public.customers"
        - "public.orders"
```

## Examples

Render Cockroach SQL into the default output directory:

```bash
./scripts/generate-cockroach-setup-sql.sh ./cockroach-setup-config.yml
```

Preview PostgreSQL grants without writing files:

```bash
./scripts/generate-postgres-grants-sql.sh --dry-run ./postgres-grants-config.yml
```

Render into a custom directory:

```bash
./scripts/generate-postgres-grants-sql.sh --output-dir ./tmp/generated-sql ./postgres-grants-config.yml
```

## Output Files

Cockroach output names:

- `cockroach-<database>-setup.sql`
- `cockroach-all-setup.sql`

PostgreSQL output names:

- `postgres-<database>-grants.sql`
- `postgres-all-grants.sql`

## Validation

Both scripts fail fast with a readable `error:` message when:

- the YAML file is missing
- a required key is absent
- a required dependency is not installed
- a configured CA certificate path does not exist
- a destination table name does not include `schema.table`
