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

pub struct DefaultBootstrapHarness {
    inner: CdcE2eHarness,
}

impl DefaultBootstrapHarness {
    pub fn start() -> Self {
        Self {
            inner: CdcE2eHarness::start(CdcE2eHarnessConfig {
                mapping_id: "app-a",
                source_database: "demo_a",
                destination_database: "app_a",
                destination_user: "migration_user_a",
                destination_password: "runner-secret-a",
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

    pub fn verify_default_migration(&self) {
        self.inner.verify_migration();
    }
}
