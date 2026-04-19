use std::time::Duration;

use crate::e2e_harness::{CdcE2eHarness, CdcE2eHarnessConfig};

const SOURCE_SETUP_SQL: &str = r#"
CREATE DATABASE demo_a;
USE demo_a;
CREATE TABLE public.customers (
    id INT8 PRIMARY KEY,
    email STRING NOT NULL
);
CREATE TABLE public.order_items (
    order_id INT8 NOT NULL,
    line_id INT8 NOT NULL,
    sku STRING NOT NULL,
    quantity INT8 NOT NULL,
    PRIMARY KEY (order_id, line_id)
);
CREATE TABLE public.audit_events (
    event_id INT8 PRIMARY KEY,
    event_type STRING NOT NULL,
    details STRING NOT NULL
);
INSERT INTO public.customers (id, email) VALUES
    (1, 'alice@example.com'),
    (2, 'bob@example.com');
INSERT INTO public.order_items (order_id, line_id, sku, quantity) VALUES
    (100, 1, 'starter-kit', 2),
    (100, 2, 'bonus-widget', 1);
INSERT INTO public.audit_events (event_id, event_type, details) VALUES
    (1, 'bootstrap', 'created before migration');
"#;

const DESTINATION_SETUP_SQL: &str = r#"
CREATE TABLE public.customers (
    id bigint PRIMARY KEY,
    email text NOT NULL
);
CREATE TABLE public.order_items (
    order_id bigint NOT NULL,
    line_id bigint NOT NULL,
    sku text NOT NULL,
    quantity bigint NOT NULL,
    PRIMARY KEY (order_id, line_id)
);
CREATE TABLE public.audit_events (
    event_id bigint PRIMARY KEY,
    event_type text NOT NULL,
    details text NOT NULL
);
"#;

const CUSTOMERS_SNAPSHOT_SQL: &str = r#"
SELECT COALESCE(
    string_agg(id::text || ':' || email, ',' ORDER BY id),
    '<empty>'
)
FROM public.customers;
"#;

const ORDER_ITEMS_SNAPSHOT_SQL: &str = r#"
SELECT COALESCE(
    string_agg(
        order_id::text || '|' || line_id::text || '|' || sku || '|' || quantity::text,
        ',' ORDER BY order_id, line_id
    ),
    '<empty>'
)
FROM public.order_items;
"#;

const AUDIT_EVENTS_SNAPSHOT_SQL: &str = r#"
SELECT COALESCE(
    string_agg(
        event_id::text || ':' || event_type || ':' || details,
        ',' ORDER BY event_id
    ),
    '<empty>'
)
FROM public.audit_events;
"#;

const HELPER_SHADOW_CUSTOMERS_SNAPSHOT_SQL: &str = r#"
SELECT COALESCE(
    string_agg(id::text || ':' || email, ',' ORDER BY id),
    '<empty>'
)
FROM _cockroach_migration_tool."app-a__public__customers";
"#;

const HELPER_SHADOW_ORDER_ITEMS_SNAPSHOT_SQL: &str = r#"
SELECT COALESCE(
    string_agg(
        order_id::text || '|' || line_id::text || '|' || sku || '|' || quantity::text,
        ',' ORDER BY order_id, line_id
    ),
    '<empty>'
)
FROM _cockroach_migration_tool."app-a__public__order_items";
"#;

const EXPECTED_HELPER_TABLES: &str =
    "app-a__public__customers,app-a__public__order_items,stream_state,table_sync_state";
const INITIAL_CUSTOMERS: &str = "1:alice@example.com,2:bob@example.com";
const INITIAL_ORDER_ITEMS: &str = "100|1|starter-kit|2,100|2|bonus-widget|1";
const LIVE_CUSTOMERS: &str = "1:alice+vip@example.com,3:carol@example.com";
const LIVE_ORDER_ITEMS: &str = "100|1|starter-kit-v2|5,101|1|replacement-kit|3";
const EXCLUDED_DESTINATION_AUDIT_EVENTS: &str = "<empty>";

pub struct CompositePkExclusionHarness {
    inner: CdcE2eHarness,
}

impl CompositePkExclusionHarness {
    pub fn start() -> Self {
        Self {
            inner: CdcE2eHarness::start(CdcE2eHarnessConfig {
                mapping_id: "app-a",
                source_database: "demo_a",
                destination_database: "app_a",
                destination_user: "migration_user_a",
                destination_password: "runner-secret-a",
                reconcile_interval_secs: 1,
                selected_tables: &["public.customers", "public.order_items"],
                source_setup_sql: SOURCE_SETUP_SQL,
                destination_setup_sql: DESTINATION_SETUP_SQL,
            }),
        }
    }

    pub fn bootstrap_migration(&self) {
        self.inner.bootstrap_migration();
    }

    pub fn wait_for_initial_scan(&self) {
        self.wait_for_included_state(INITIAL_CUSTOMERS, INITIAL_ORDER_ITEMS, "initial scan");
        self.inner.wait_for_destination_query(
            AUDIT_EVENTS_SNAPSHOT_SQL,
            EXCLUDED_DESTINATION_AUDIT_EVENTS,
            "excluded destination audit events after initial scan",
        );
        self.inner.wait_for_helper_tables(
            EXPECTED_HELPER_TABLES,
            "helper table inventory for selected tables",
        );
    }

    pub fn assert_explicit_source_bootstrap_commands(&self) {
        self.inner.assert_explicit_source_bootstrap_commands();
    }

    pub fn apply_live_source_changes(&self) {
        self.inner.apply_source_workload_batch(
            r#"
UPDATE public.customers
SET email = 'alice+vip@example.com'
WHERE id = 1;
DELETE FROM public.customers
WHERE id = 2;
INSERT INTO public.customers (id, email)
VALUES (3, 'carol@example.com');

UPDATE public.order_items
SET sku = 'starter-kit-v2', quantity = 5
WHERE order_id = 100 AND line_id = 1;
DELETE FROM public.order_items
WHERE order_id = 100 AND line_id = 2;
INSERT INTO public.order_items (order_id, line_id, sku, quantity)
VALUES (101, 1, 'replacement-kit', 3);

UPDATE public.audit_events
SET details = 'mutated after bootstrap'
WHERE event_id = 1;
INSERT INTO public.audit_events (event_id, event_type, details)
VALUES (2, 'live-write', 'should stay excluded');
"#,
        );
    }

    pub fn wait_for_live_catchup(&self) {
        self.wait_for_included_state(LIVE_CUSTOMERS, LIVE_ORDER_ITEMS, "live catch-up");
        self.inner.wait_for_destination_query(
            AUDIT_EVENTS_SNAPSHOT_SQL,
            EXCLUDED_DESTINATION_AUDIT_EVENTS,
            "excluded destination audit events after live writes",
        );
        self.inner.wait_for_helper_tables(
            EXPECTED_HELPER_TABLES,
            "helper table inventory after live writes",
        );
    }

    pub fn assert_included_tables_stable(&self, duration: Duration) {
        self.inner.assert_destination_query_stable(
            CUSTOMERS_SNAPSHOT_SQL,
            LIVE_CUSTOMERS,
            "destination customers after repeated reconcile",
            duration,
        );
        self.inner.assert_destination_query_stable(
            ORDER_ITEMS_SNAPSHOT_SQL,
            LIVE_ORDER_ITEMS,
            "destination order items after repeated reconcile",
            duration,
        );
        self.inner.assert_destination_query_stable(
            HELPER_SHADOW_CUSTOMERS_SNAPSHOT_SQL,
            LIVE_CUSTOMERS,
            "helper shadow customers after repeated reconcile",
            duration,
        );
        self.inner.assert_destination_query_stable(
            HELPER_SHADOW_ORDER_ITEMS_SNAPSHOT_SQL,
            LIVE_ORDER_ITEMS,
            "helper shadow order items after repeated reconcile",
            duration,
        );
        self.inner.assert_destination_query_stable(
            AUDIT_EVENTS_SNAPSHOT_SQL,
            EXCLUDED_DESTINATION_AUDIT_EVENTS,
            "excluded destination audit events after repeated reconcile",
            duration,
        );
        assert_eq!(
            self.inner.helper_tables().trim(),
            EXPECTED_HELPER_TABLES,
            "excluded tables must never materialize helper shadow tables",
        );
    }

    fn wait_for_included_state(
        &self,
        expected_customers: &str,
        expected_order_items: &str,
        description: &str,
    ) {
        self.inner.wait_for_destination_query(
            CUSTOMERS_SNAPSHOT_SQL,
            expected_customers,
            &format!("destination customers during {description}"),
        );
        self.inner.wait_for_destination_query(
            ORDER_ITEMS_SNAPSHOT_SQL,
            expected_order_items,
            &format!("destination order items during {description}"),
        );
        self.inner.wait_for_destination_query(
            HELPER_SHADOW_CUSTOMERS_SNAPSHOT_SQL,
            expected_customers,
            &format!("helper shadow customers during {description}"),
        );
        self.inner.wait_for_destination_query(
            HELPER_SHADOW_ORDER_ITEMS_SNAPSHOT_SQL,
            expected_order_items,
            &format!("helper shadow order items during {description}"),
        );
        self.inner.wait_for_helper_table_row_counts(&[
            ("public.customers", expected_customers.split(',').count()),
            (
                "public.order_items",
                expected_order_items.split(',').count(),
            ),
        ]);
    }
}
