# CockroachDB Source Setup

Use this guide to prepare the SQL that runs on the source CockroachDB cluster before `runner` starts. The goal is simple:

1. Turn on rangefeeds so CockroachDB can emit changefeed events.
2. Capture one starting cursor per source database.
3. Create one webhook changefeed per mapping, targeting `runner` at `/ingest/<mapping_id>`.

The runtime assumes these changefeeds already exist. It does not create them for you.

## Run This Guide On

- The source CockroachDB cluster
- A Cockroach SQL client session with permission to change cluster settings and create changefeeds

## Prerequisites

- You know the CockroachDB SQL endpoint you will connect to as `{{ cockroach_url }}`.
- You know the source database for each mapping as `{{ database }}`.
- You know every mapped source table as `{{ schema }}` plus `{{ table }}`.
- You know the public HTTPS base URL for `runner` as `{{ webhook_base_url }}`.
- You have the PEM-encoded CA certificate that CockroachDB must trust when posting to the runner webhook, and you can derive `{{ ca_cert_base64 }}` from it.
- You have chosen a changefeed resolved watermark cadence for `{{ resolved_interval }}` such as `1s` or `5s`.
- You know the stable mapping identifier `{{ mapping_id }}` used in the runner config.

## SQL Templates

### 1. Enable Rangefeeds Once Per Cluster

Run this once on the source cluster before any changefeed creation:

```sql
SET CLUSTER SETTING kv.rangefeed.enabled = true;
```

What it does:

- Enables the rangefeed capability that CockroachDB changefeeds depend on.

Prerequisites:

- Cluster-level permission to change cluster settings.

### 2. Capture One Cursor Per Source Database

Run this once per source database, immediately before creating the changefeeds that should start from the same point in time:

```sql
-- Connect your SQL client to {{ cockroach_url }} first.
USE {{ database }};
SELECT cluster_logical_timestamp() AS changefeed_cursor;
```

What it does:

- Switches the SQL session to the source database.
- Returns a decimal cursor value that pins the starting point for all changefeeds you create from that capture.

Prerequisites:

- The source database already exists.
- You can connect to the source cluster and run `SELECT cluster_logical_timestamp()`.

### 3. Create One Changefeed Per Mapping

Use the cursor you just captured for every mapping in that source database that should begin from the same consistent point:

```sql
-- Connect your SQL client to {{ cockroach_url }} first.
-- Replace __PASTE_CAPTURED_CURSOR_HERE__ with the value returned by the cursor query above.
CREATE CHANGEFEED FOR TABLE
{% for selected_table in selected_tables -%}
{{ database }}.{{ schema }}.{{ table }}{% if not loop.last %}, {% endif %}
{%- endfor %}
INTO 'webhook-https://{{ webhook_base_url }}/ingest/{{ mapping_id }}?ca_cert={{ ca_cert_base64 }}'
WITH cursor = '__PASTE_CAPTURED_CURSOR_HERE__',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '{{ resolved_interval }}';
```

What it does:

- Creates one changefeed job for the selected tables in a single mapping.
- Uses fully qualified table names in the form `database.schema.table`.
- Sends webhook batches to `/ingest/{{ mapping_id }}` on `runner`.
- Forces an initial snapshot with `initial_scan = 'yes'`.
- Uses the enriched webhook shape the runtime expects with `envelope = 'enriched'`.
- Emits resolved watermarks on the configured cadence.

Prerequisites:

- Every table named in the `CREATE CHANGEFEED` statement already exists in the source database.
- The runner HTTPS endpoint is reachable from the source cluster.
- The `ca_cert` query parameter value is already safe to place in a URL query string. If you start from raw base64 output, percent-encode it before substitution.

## Variable Reference

| Placeholder | Meaning | How to determine it | Example |
| --- | --- | --- | --- |
| `{{ cockroach_url }}` | The CockroachDB SQL endpoint the operator connects to for manual execution. This is operator context, not a clause inside `CREATE CHANGEFEED`. | Use the SQL connection string or host and port for the source cluster. | `postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require` |
| `{{ webhook_base_url }}` | The base HTTPS URL for `runner`, before `/ingest/{{ mapping_id }}` is appended. | Use the externally reachable runner address and port. Do not include the mapping-specific ingest suffix yourself. | `runner.example.internal:8443` |
| `{{ ca_cert_base64 }}` | The CA certificate bytes encoded for the `ca_cert` query parameter on the webhook sink URL. | Start from the PEM file the source cluster should trust. Base64-encode the bytes, then percent-encode that base64 text before you place it in the URL. | `LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D` |
| `{{ resolved_interval }}` | The resolved watermark cadence passed to CockroachDB. | Choose an interval that matches how often you want resolved events, then keep it consistent with your operator expectations. | `1s` |
| `{{ database }}` | The source CockroachDB database for the mapping being rendered. | Copy the `mappings[].source.database` value from the runner config or your SQL generation input. | `demo_a` |
| `{{ schema }}` | The schema portion of one mapped source table. | Split each selected table name on the first dot. For `public.customers`, the schema is `public`. | `public` |
| `{{ table }}` | The table portion of one mapped source table. | Split each selected table name on the first dot. For `public.customers`, the table is `customers`. | `customers` |
| `{{ mapping_id }}` | The stable mapping identifier used by `runner` and by the ingest path. | Copy the `mappings[].id` value from the runner config. | `app-a` |
| `selected_tables` | The ordered list of source tables that should be included in one mapping's `CREATE CHANGEFEED` statement. | Group the fully qualified source tables for one mapping. Each entry supplies `{{ schema }}` and `{{ table }}` while `{{ database }}` stays fixed for the statement. | `public.customers`, `public.orders` |

## Statement-By-Statement Notes

### Why capture the cursor once?

- `cluster_logical_timestamp()` returns a specific starting point in the source database timeline.
- Capture it once, then reuse that exact value in every `CREATE CHANGEFEED` statement for the mappings that belong to that database.
- Reusing one captured value keeps those changefeeds aligned on the same start boundary. That matters when you want a consistent initial scan plus live replay window.

### Where do you paste the cursor?

- Paste the decimal value returned by `SELECT cluster_logical_timestamp() AS changefeed_cursor;` into the `cursor = '...'` clause of each `CREATE CHANGEFEED` statement for that database.
- Do not run a second cursor query between mappings unless you explicitly want later mappings to start from a different logical time.

### Why is the ingest path fixed?

- The runtime route contract is `/ingest/<mapping_id>`.
- The code path that owns this shape trims any trailing slash from the base URL and appends `/ingest/{{ mapping_id }}`.
- If your runner base URL already contains a path prefix, keep that prefix in `{{ webhook_base_url }}` and still let the final `/ingest/{{ mapping_id }}` suffix be appended exactly once.

## Worked Example: Two Databases, Two Mappings

This example shows one captured cursor per source database and one changefeed per mapping.

```sql
-- Connect with: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require

SET CLUSTER SETTING kv.rangefeed.enabled = true;

-- Database demo_a
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;
-- Suppose the query returned 1745877420457561000.0000000000.
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders
INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D'
WITH cursor = '1745877420457561000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';

-- Database demo_b
USE demo_b;
SELECT cluster_logical_timestamp() AS changefeed_cursor;
-- Suppose the query returned 1745877420459999000.0000000000.
CREATE CHANGEFEED FOR TABLE demo_b.public.invoices, demo_b.public.invoice_lines
INTO 'webhook-https://runner.example.internal:8443/ingest/app-b?ca_cert=LS0tLS1CRUdJTiBDRVJUSUZJQ0FURS0t...%3D%3D'
WITH cursor = '1745877420459999000.0000000000',
     initial_scan = 'yes',
     envelope = 'enriched',
     resolved = '5s';
```

What this example demonstrates:

- `kv.rangefeed.enabled` is cluster-wide, so it appears once.
- Each source database gets its own cursor capture because `USE {{ database }}` changes the session database before calling `cluster_logical_timestamp()`.
- Each mapping produces its own `/ingest/{{ mapping_id }}` sink and its own `CREATE CHANGEFEED` statement.
- The table list is fully qualified as `database.schema.table` for every selected table.

## Operator Review Checklist

- The runner endpoint in the sink URL ends in `/ingest/{{ mapping_id }}`.
- The `ca_cert` query parameter contains URL-safe certificate data.
- The `cursor` value came from the matching source database and was captured once before the changefeed creation for that database.
- The selected tables are fully qualified and belong to the same source database used in the cursor capture step.
- The changefeed options are exactly `initial_scan = 'yes'`, `envelope = 'enriched'`, and `resolved = '{{ resolved_interval }}'`.
