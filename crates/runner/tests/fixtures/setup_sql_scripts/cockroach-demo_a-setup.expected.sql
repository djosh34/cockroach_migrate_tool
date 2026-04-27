-- Source bootstrap SQL
-- Cockroach URL: postgresql://root@crdb.example.internal:26257/defaultdb?sslmode=require
-- Apply each statement with a Cockroach SQL client against the source cluster.
-- Capture the cursor once, then replace __CHANGEFEED_CURSOR__ in the CREATE CHANGEFEED statements below.

SET CLUSTER SETTING kv.rangefeed.enabled = true;

-- Source database: demo_a
USE demo_a;
SELECT cluster_logical_timestamp() AS changefeed_cursor;

-- Mapping: app-a
-- Selected tables: public.customers, public.orders
-- Replace __CHANGEFEED_CURSOR__ below with the decimal cursor returned above before running the CREATE CHANGEFEED statement.
CREATE CHANGEFEED FOR TABLE demo_a.public.customers, demo_a.public.orders INTO 'webhook-https://runner.example.internal:8443/ingest/app-a?ca_cert=__CA_CERT_BASE64__' WITH cursor = '__CHANGEFEED_CURSOR__', initial_scan = 'yes', envelope = 'enriched', enriched_properties = 'source', resolved = '5s';
