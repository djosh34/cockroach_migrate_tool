# Source CockroachDB Setup

Before starting the runner, you must prepare the source CockroachDB cluster. The runner receives webhook payloads — it does not create changefeeds for you.

## Three steps

1. **Enable rangefeeds** — cluster-wide setting, run once per cluster.
2. **Capture a cursor** — one `cluster_logical_timestamp()` per source database.
3. **Create changefeeds** — one `CREATE CHANGEFEED` per mapping, each targeting the runner's `/ingest/{mapping_id}` endpoint.

## Detailed guide

The full walkthrough with SQL examples, sink URL encoding, and a pre-flight checklist:

**[CockroachDB source setup](./source-setup/cockroachdb-setup.md)**

## Quick reference

```sql
-- 1. Enable rangefeeds (once per cluster)
SET CLUSTER SETTING kv.rangefeed.enabled = true;

-- 2. Capture cursor (once per source database)
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;

-- 3. Create changefeed (one per mapping)
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders
  INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=PERCENT_ENCODED_BASE64_CA_CERT'
  WITH
    cursor = '1745877420457561000.0000000000',
    initial_scan = 'yes',
    envelope = 'enriched',
    resolved = '5s';
```

See the [detailed guide](./source-setup/cockroachdb-setup.md) for sink URL encoding, HTTP sink configuration, and the complete order-of-operations checklist.
