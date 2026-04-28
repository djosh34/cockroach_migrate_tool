# Destination PostgreSQL Grants

The runtime role needs specific permissions on each destination PostgreSQL database. The runner creates its own `_cockroach_migration_tool` helper schema after these grants are in place.

## Three steps

1. **Database-level** — `GRANT CONNECT, CREATE` so the role can connect and create the helper schema.
2. **Schema-level** — `GRANT USAGE` on each mapped schema.
3. **Table-level** — `GRANT SELECT, INSERT, UPDATE, DELETE` on each mapped table.

## Detailed guide

The full walkthrough with per-database examples, multi-mapping examples, and a pre-flight checklist:

**[PostgreSQL destination grants](./destination-setup/postgresql-grants.md)**

## Quick reference

```sql
-- Per destination database (replace <database> and <runtime_role>)
GRANT CONNECT, CREATE ON DATABASE <database> TO <runtime_role>;

-- Per mapped schema
GRANT USAGE ON SCHEMA <schema> TO <runtime_role>;

-- Per mapped table
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE <schema>.<table> TO <runtime_role>;
```

> **Important:** The runner does not create destination databases, schemas, or tables. Those must exist before the runner starts.

See the [detailed guide](./destination-setup/postgresql-grants.md) for worked examples and the complete checklist.