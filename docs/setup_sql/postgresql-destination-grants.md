# PostgreSQL Destination Grants

Use this guide to prepare the PostgreSQL privileges that `runner` needs before it starts. The runtime bootstraps its own helper schema and tracking tables, but it can only do that after the destination role has the right database, schema, and table grants.

## Run This Guide On

- The destination PostgreSQL server
- The destination database for each mapping
- A SQL client session running as the database owner, schema owner, table owner, or a superuser with permission to issue the grants

## Prerequisites

- The destination database already exists as `{{ database }}`.
- The runtime login role already exists as `{{ runtime_role }}`.
- Every mapped destination schema `{{ schema }}` already exists.
- Every mapped destination table `{{ schema }}.{{ table }}` already exists.

## SQL Templates

### 1. Grant Database Access And Helper-Schema Creation

Run this once per destination database and runtime role:

```sql
GRANT CONNECT, CREATE ON DATABASE {{ database }} TO {{ runtime_role }};
```

What it does:

- Allows the runtime to connect to the destination database.
- Allows the runtime to create `_cockroach_migration_tool` inside that database.

Prerequisites:

- `{{ database }}` already exists.
- The grant executor has permission to grant privileges on that database.

### 2. Grant Schema Usage

Run this once per mapped destination schema and runtime role:

```sql
GRANT USAGE ON SCHEMA {{ schema }} TO {{ runtime_role }};
```

What it does:

- Allows the runtime role to reference tables inside the mapped schema.

Prerequisites:

- `{{ schema }}` already exists in `{{ database }}`.

### 3. Grant Table DML Privileges

Run this once per mapped destination table and runtime role:

```sql
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE {{ schema }}.{{ table }} TO {{ runtime_role }};
```

What it does:

- Allows the runtime to read destination rows, insert new rows, update existing rows, and delete rows during reconciliation.

Prerequisites:

- `{{ schema }}.{{ table }}` already exists in `{{ database }}`.

## Variable Reference

| Placeholder | Meaning | How to determine it | Example |
| --- | --- | --- | --- |
| `{{ database }}` | The destination PostgreSQL database for the mapping or mapping group. | Copy the `mappings[].destination.database` value from the runner config or your SQL generation input. | `app_a` |
| `{{ runtime_role }}` | The PostgreSQL login role that `runner` uses to connect. | Copy the role name from the destination connection configuration. | `migration_user_a` |
| `{{ schema }}` | The destination schema that contains a mapped table. | Split each mapped table name on the first dot. For `public.customers`, the schema is `public`. | `public` |
| `{{ table }}` | The destination table that receives reconciled rows. | Split each mapped table name on the first dot. For `public.customers`, the table is `customers`. | `customers` |

## Why These Grants Are Sufficient

- `CONNECT` lets the runtime log into the destination database.
- `CREATE` on the database lets the runtime create `_cockroach_migration_tool`.
- `USAGE` on the mapped schema lets the runtime resolve the destination tables by name.
- `SELECT, INSERT, UPDATE, DELETE` on each mapped real table lets reconciliation read and mutate destination rows.

The runtime creates these helper objects itself after the grants are in place:

- Schema `_cockroach_migration_tool`
- Table `_cockroach_migration_tool.stream_state`
- Table `_cockroach_migration_tool.table_sync_state`
- One helper table per mapping and selected source table

You do not grant privileges on `_cockroach_migration_tool` ahead of time because the runtime role creates that schema and therefore owns the objects it creates there.

## Worked Example: Two Databases, Two Mappings

This example uses one runtime role per destination database.

```sql
-- Destination database app_a
GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;
GRANT USAGE ON SCHEMA public TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.orders TO migration_user_a;

-- Destination database app_b
GRANT CONNECT, CREATE ON DATABASE app_b TO migration_user_b;
GRANT USAGE ON SCHEMA public TO migration_user_b;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.invoices TO migration_user_b;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.invoice_lines TO migration_user_b;
```

What this example demonstrates:

- The database-level grant is once per database and runtime role.
- The schema grant is once per schema and runtime role.
- The table grant is once per mapped table and runtime role.
- If multiple mappings land in the same destination database with the same runtime role, deduplicate identical `GRANT` statements rather than issuing the same line repeatedly.

## Operator Review Checklist

- `{{ runtime_role }}` can connect to `{{ database }}`.
- `{{ runtime_role }}` can create `_cockroach_migration_tool` because it has `CREATE` on `{{ database }}`.
- Every mapped schema has a matching `GRANT USAGE ON SCHEMA ...`.
- Every mapped destination table has a matching `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE ...`.
- No extra privileges are assumed for bootstrap. If you need broader operational privileges, treat them as a separate database policy decision.
