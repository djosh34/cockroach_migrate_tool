use std::time::Duration;

use crate::e2e_harness::{CdcE2eHarness, CdcE2eHarnessConfig};

const DEFAULT_SOURCE_SETUP_SQL: &str = r#"
CREATE DATABASE demo_a;
USE demo_a;
CREATE TABLE public.customers (
    id INT8 PRIMARY KEY,
    email STRING NOT NULL
);
INSERT INTO public.customers (id, email) VALUES
    (1, 'alice@example.com'),
    (2, 'bob@example.com');
"#;

const DEFAULT_DESTINATION_SETUP_SQL: &str = r#"
CREATE TABLE public.customers (
    id bigint PRIMARY KEY,
    email text NOT NULL
);
"#;

const DEFAULT_CUSTOMERS_SNAPSHOT_SQL: &str = r#"
SELECT COALESCE(
    string_agg(id::text || ':' || email, ',' ORDER BY id),
    '<empty>'
)
FROM public.customers;
"#;

const HELPER_SHADOW_CUSTOMERS_SNAPSHOT_SQL: &str = r#"
SELECT COALESCE(
    string_agg(id::text || ':' || email, ',' ORDER BY id),
    '<empty>'
)
FROM _cockroach_migration_tool."app-a__public__customers";
"#;

pub struct DefaultBootstrapHarness {
    inner: CdcE2eHarness,
}

impl DefaultBootstrapHarness {
    pub fn start() -> Self {
        Self::start_with_reconcile_interval(1)
    }

    pub fn start_with_reconcile_interval(reconcile_interval_secs: u64) -> Self {
        Self {
            inner: CdcE2eHarness::start(CdcE2eHarnessConfig {
                mapping_id: "app-a",
                source_database: "demo_a",
                destination_database: "app_a",
                destination_user: "migration_user_a",
                destination_password: "runner-secret-a",
                reconcile_interval_secs,
                selected_tables: &["public.customers"],
                source_setup_sql: DEFAULT_SOURCE_SETUP_SQL,
                destination_setup_sql: DEFAULT_DESTINATION_SETUP_SQL,
            }),
        }
    }

    pub fn bootstrap_default_migration(&self) {
        self.inner.bootstrap_migration();
    }

    pub fn wait_for_destination_customers(&self, expected: &str) {
        self.inner.wait_for_destination_query(
            DEFAULT_CUSTOMERS_SNAPSHOT_SQL,
            expected,
            "destination customers",
        );
    }

    pub fn assert_explicit_source_bootstrap_commands(&self) {
        self.inner.assert_explicit_source_bootstrap_commands();
    }

    pub fn assert_helper_shadow_customers(&self, expected_rows: usize) {
        assert_eq!(
            self.inner.helper_table_row_count("public.customers"),
            expected_rows,
            "helper shadow table should contain the initial scan rows"
        );
    }

    pub fn wait_for_helper_shadow_customers(&self, expected: &str) {
        self.inner.wait_for_destination_query(
            HELPER_SHADOW_CUSTOMERS_SNAPSHOT_SQL,
            expected,
            "helper shadow customers",
        );
    }

    pub fn assert_destination_customer_count(&self, customer_id: i64, expected_count: usize) {
        let actual = self
            .inner
            .query_destination(&format!(
                "SELECT count(*)::text FROM public.customers WHERE id = {customer_id};"
            ))
            .trim()
            .parse::<usize>()
            .expect("destination customer count should parse");
        assert_eq!(
            actual, expected_count,
            "destination customers should contain {expected_count} row(s) for id {customer_id}"
        );
    }

    pub fn assert_helper_shadow_customers_stable(&self, expected: &str, duration: Duration) {
        self.inner.assert_destination_query_stable(
            HELPER_SHADOW_CUSTOMERS_SNAPSHOT_SQL,
            expected,
            "helper shadow customers",
            duration,
        );
    }

    pub fn assert_destination_customers_stable(&self, expected: &str, duration: Duration) {
        self.inner.assert_destination_query_stable(
            DEFAULT_CUSTOMERS_SNAPSHOT_SQL,
            expected,
            "destination customers",
            duration,
        );
    }

    pub fn delete_source_customer(&self, customer_id: i64) {
        self.inner
            .execute_source_sql(&format!("DELETE FROM public.customers WHERE id = {customer_id};"));
    }

    pub fn verify_default_migration(&self) {
        let _ = self.verify_default_migration_output();
    }

    pub fn verify_default_migration_output(&self) -> String {
        self.inner.verify_migration()
    }
}
