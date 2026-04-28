# PostgreSQL Destination Grants

You must grant the runtime role access to each destination PostgreSQL database before starting the runner. The runner creates its own helper schema and tracking tables after these grants are in place.

## What you need

- PostgreSQL SQL access as a database owner, schema owner, table owner, or superuser.
- The destination database name for each mapping.
- The runtime login role that the runner uses to connect.
- Every mapped destination schema and table.

> **The runner does not create the destination database, schemas, or tables.** It only creates the `_cockroach_migration_tool` helper schema and its internal tracking tables.

## Step 1: Grant database access

Run once per destination database and runtime role:

```sql
GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;
```

- `CONNECT` lets the runtime log into the database.
- `CREATE` lets the runtime create `_cockroach_migration_tool` inside that database.

## Step 2: Grant schema usage

Run once per mapped destination schema and runtime role:

```sql
GRANT USAGE ON SCHEMA public TO migration_user_a;
```

## Step 3: Grant table DML privileges

Run once per mapped destination table and runtime role:

```sql
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;
GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.orders TO migration_user_a;
```

| Privilege | Why the runner needs it |
| --------- | ----------------------- |
| `SELECT` | Check existing rows during reconciliation |
| `INSERT` | Write new rows from changefeed events |
| `UPDATE` | Update existing rows when changefeed events carry modifications |
| `DELETE` | Delete rows when changefeed events carry deletion payloads |

## Worked example: two databases, two mappings

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

## What the runner creates

After the grants are in place, the runtime creates these objects automatically:

- Schema: `_cockroach_migration_tool`
- Table: `_cockroach_migration_tool.stream_state`
- Table: `_cockroach_migration_tool.table_sync_state`
- Additional helper tables per mapping and mapped source table

You do not need to grant privileges on `_cockroach_migration_tool` ahead of time because the runtime role owns the objects it creates there.

## Checklist

- [ ] Every destination database has `GRANT CONNECT, CREATE ON DATABASE <database> TO <runtime_role>`.
- [ ] Every mapped schema has `GRANT USAGE ON SCHEMA <schema> TO <runtime_role>`.
- [ ] Every mapped table has `GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE <schema>.<table> TO <runtime_role>`.
- [ ] The runtime role exists and can authenticate against the destination PostgreSQL server.
- [ ] The destination databases, schemas, and tables all exist before the runner starts.

> **Adding new tables?** If you add new tables to a mapping, you must grant privileges on those tables before restarting the runner. The runner does not attempt to grant privileges itself.

## See also

- [Runner getting started](../runner/getting-started.md) — write runner config and start the runner
- [TLS reference](../tls-reference.md) — runner destination TLS configuration
- [Troubleshooting](../troubleshooting.md) — common destination connectivity errors