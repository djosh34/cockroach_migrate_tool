use std::time::Duration;

use crate::e2e_harness::{
    CdcE2eHarness, CdcE2eHarnessConfig, DestinationTableLock, MappingTrackingProgress,
    WebhookSinkMode,
};
use crate::webhook_chaos_gateway::ExternalSinkFault;

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
            inner: Self::build_inner(reconcile_interval_secs, WebhookSinkMode::DirectRunner),
        }
    }

    pub fn start_with_external_sink_faults() -> Self {
        Self {
            inner: Self::build_inner(1, WebhookSinkMode::ExternalChaosGateway),
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
        self.inner.execute_source_sql(&format!(
            "DELETE FROM public.customers WHERE id = {customer_id};"
        ));
    }

    pub fn arm_single_external_http_500_for_customer_email(&self, email: &str) {
        self.inner
            .arm_single_external_sink_fault_for_request_body(
                email,
                ExternalSinkFault::HttpStatus {
                    status: axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                },
            );
    }

    pub fn arm_single_external_transport_disconnect_for_customer_email(&self, email: &str) {
        self.inner.arm_single_external_sink_fault_for_request_body(
            email,
            ExternalSinkFault::AbortConnectionBeforeForward,
        );
    }

    pub fn update_source_customer_email(&self, customer_id: i64, email: &str) {
        self.inner.execute_source_sql(&format!(
            "UPDATE public.customers
             SET email = '{email}'
             WHERE id = {customer_id};",
            email = email.replace('\'', "''"),
        ));
    }

    pub fn wait_for_duplicate_customer_delivery(&self, email: &str) {
        self.inner
            .wait_for_duplicate_gateway_delivery_of_request_body(email);
    }

    pub fn wait_for_gateway_transport_abort_then_success_for_customer_email(&self, email: &str) {
        self.inner.wait_for_gateway_fault_then_success_for_request_body(
            email,
            ExternalSinkFault::AbortConnectionBeforeForward,
        );
    }

    pub fn verify_default_migration(&self) {
        let _ = self.verify_default_migration_output();
    }

    pub fn verify_default_migration_output(&self) -> String {
        self.inner.verify_migration()
    }

    pub fn kill_runner(&self) {
        self.inner.kill_runner();
    }

    pub fn restart_runner(&self) {
        self.inner.restart_runner();
    }

    pub fn customer_tracking_progress(&self) -> MappingTrackingProgress {
        self.inner.tracking_progress("public.customers")
    }

    pub fn wait_for_customer_tracking_progress<F>(
        &self,
        description: &str,
        predicate: F,
    ) -> MappingTrackingProgress
    where
        F: FnMut(&MappingTrackingProgress) -> bool,
    {
        self.inner
            .wait_for_tracking_progress("public.customers", description, predicate)
    }

    pub fn lock_destination_customers(&self) -> DestinationTableLock {
        self.inner.lock_destination_table("public.customers")
    }

    pub fn wait_for_customer_reconcile_block(&self) {
        self.inner
            .wait_for_reconcile_block_on_destination_table("public.customers");
    }

    fn build_inner(
        reconcile_interval_secs: u64,
        webhook_sink_mode: WebhookSinkMode,
    ) -> CdcE2eHarness {
        CdcE2eHarness::start_with_webhook_sink(
            CdcE2eHarnessConfig {
                mapping_id: "app-a",
                source_database: "demo_a",
                destination_database: "app_a",
                destination_user: "migration_user_a",
                destination_password: "runner-secret-a",
                reconcile_interval_secs,
                selected_tables: &["public.customers"],
                source_setup_sql: DEFAULT_SOURCE_SETUP_SQL,
                destination_setup_sql: DEFAULT_DESTINATION_SETUP_SQL,
            },
            webhook_sink_mode,
        )
    }
}
