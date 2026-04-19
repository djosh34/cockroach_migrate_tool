use std::time::Duration;

use crate::e2e_harness::{
    CdcE2eHarness, CdcE2eHarnessConfig, DestinationTableLock, DestinationWriteFailure,
    MappingTrackingProgress, WebhookSinkMode,
};
use crate::e2e_integrity::{
    CustomerLiveUpdateAudit, DestinationRuntimeMode, PostSetupSourceAudit, RuntimeShapeAudit,
    VerifyCorrectnessAudit,
};
use crate::verify_image_harness_support::VerifyImageHarness;
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

const HELPER_SHADOW_CUSTOMERS_SNAPSHOT_SQL: &str = r#"
SELECT COALESCE(
    string_agg(id::text || ':' || email, ',' ORDER BY id),
    '<empty>'
)
FROM _cockroach_migration_tool."app-a__public__customers";
"#;

const HIGH_SOURCE_CUSTOMER_WRITE_CHURN_SQL: &str = r#"
UPDATE public.customers
SET email = 'alice+churn-1@example.com'
WHERE id = 1;
INSERT INTO public.customers (id, email) VALUES
    (3, 'carol+new@example.com'),
    (4, 'dora+new@example.com'),
    (5, 'erin+new@example.com');
UPDATE public.customers
SET email = 'carol+updated@example.com'
WHERE id = 3;
DELETE FROM public.customers
WHERE id = 3;
INSERT INTO public.customers (id, email) VALUES
    (3, 'carol+reborn@example.com');
UPDATE public.customers
SET email = 'bob+churn-1@example.com'
WHERE id = 2;
UPDATE public.customers
SET email = 'dora+steady@example.com'
WHERE id = 4;
UPDATE public.customers
SET email = 'erin+updated@example.com'
WHERE id = 5;
DELETE FROM public.customers
WHERE id = 4;
INSERT INTO public.customers (id, email) VALUES
    (6, 'frank+ephemeral@example.com'),
    (4, 'dora+returning@example.com');
UPDATE public.customers
SET email = 'dora+steady@example.com'
WHERE id = 4;
DELETE FROM public.customers
WHERE id IN (5, 6);
INSERT INTO public.customers (id, email) VALUES
    (5, 'erin+restored@example.com');
UPDATE public.customers
SET email = 'alice+final@example.com'
WHERE id = 1;
UPDATE public.customers
SET email = 'bob+final@example.com'
WHERE id = 2;
"#;

pub struct CustomerWriteChurnExpectation {
    final_customers_snapshot: String,
}

impl CustomerWriteChurnExpectation {
    pub fn final_customers_snapshot(&self) -> &str {
        &self.final_customers_snapshot
    }
}

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

    pub fn start_with_observed_webhook_gateway() -> Self {
        Self {
            inner: Self::build_inner(1, WebhookSinkMode::ObservableChaosGateway),
        }
    }

    pub fn start_with_observed_webhook_gateway_and_reconcile_interval(
        reconcile_interval_secs: u64,
    ) -> Self {
        Self {
            inner: Self::build_inner(
                reconcile_interval_secs,
                WebhookSinkMode::ObservableChaosGateway,
            ),
        }
    }

    pub fn bootstrap_default_migration(&self) {
        self.inner.bootstrap_migration();
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

    pub fn run_high_source_customer_write_churn_workload(&self) -> CustomerWriteChurnExpectation {
        self.inner
            .apply_source_workload_batch(HIGH_SOURCE_CUSTOMER_WRITE_CHURN_SQL);
        CustomerWriteChurnExpectation {
            final_customers_snapshot:
                "1:alice+final@example.com,2:bob+final@example.com,3:carol+reborn@example.com,4:dora+steady@example.com,5:erin+restored@example.com"
                    .to_owned(),
        }
    }

    pub fn assert_helper_shadow_customers_snapshot(&self, expected: &str) {
        let actual = self
            .inner
            .query_destination(HELPER_SHADOW_CUSTOMERS_SNAPSHOT_SQL);
        assert_eq!(
            actual.trim(),
            expected,
            "helper shadow customers snapshot did not match"
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

    pub fn verify_selected_tables_via_image(
        &self,
        verify_image: &VerifyImageHarness,
    ) -> VerifyCorrectnessAudit {
        self.inner.verify_selected_tables_via_image(verify_image)
    }

    pub fn wait_for_selected_tables_to_match_via_verify_image(
        &self,
        verify_image: &VerifyImageHarness,
    ) -> VerifyCorrectnessAudit {
        self.inner.wait_for_selected_tables_to_match_via_image(
            verify_image,
            "default selected tables should converge through the verify image",
        )
    }

    pub fn assert_selected_tables_match_via_verify_image_stable(
        &self,
        verify_image: &VerifyImageHarness,
        duration: Duration,
    ) {
        self.inner.assert_selected_tables_match_via_image_stable(
            verify_image,
            "default selected tables should remain matched through the verify image",
            duration,
        );
    }

    pub fn assert_runner_alive(&self) {
        self.inner.assert_runner_alive();
    }

    pub fn delete_source_customer(&self, customer_id: i64) {
        self.inner.apply_source_workload_batch(&format!(
            "DELETE FROM public.customers WHERE id = {customer_id};"
        ));
    }

    pub fn arm_single_external_http_500_for_customer_email(&self, email: &str) {
        self.inner.arm_single_external_sink_fault_for_request_body(
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
        self.inner.apply_source_workload_batch(&format!(
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

    pub fn wait_for_customer_update_received_before_reconcile(
        &self,
        baseline: &MappingTrackingProgress,
        updated_helper_snapshot: &str,
        updated_email: &str,
    ) -> CustomerLiveUpdateAudit {
        self.inner
            .wait_for_gateway_forwarded_status_sequence_for_request_body(
                updated_email,
                &[reqwest::StatusCode::OK],
            );
        self.wait_for_helper_shadow_customers(updated_helper_snapshot);
        let progress = self.wait_for_customer_tracking_progress(
            "customer update should be durably received before reconcile catches up",
            |progress| {
                progress
                    .stream
                    .received_has_advanced_since(&baseline.stream)
                    && progress.stream.latest_reconciled_resolved_watermark
                        == baseline.stream.latest_reconciled_resolved_watermark
                    && progress.table.last_successful_sync_watermark
                        == baseline.table.last_successful_sync_watermark
                    && progress.table.last_error.is_none()
            },
        );
        let received_watermark = progress
            .stream
            .latest_received_resolved_watermark
            .expect("received-before-reconcile progress should capture a received watermark");
        CustomerLiveUpdateAudit::new(received_watermark)
    }

    pub fn wait_for_customer_update_reconcile(
        &self,
        verify_image: &VerifyImageHarness,
        audit: &CustomerLiveUpdateAudit,
    ) -> MappingTrackingProgress {
        self.wait_for_selected_tables_to_match_via_verify_image(verify_image);
        self.wait_for_customer_tracking_progress(
            "customer update should reconcile through the received watermark without storing errors",
            |progress| {
                progress.has_reconciled_through(audit.received_watermark())
                    && progress.table.last_error.is_none()
            },
        )
    }

    pub fn wait_for_gateway_transport_abort_then_success_for_customer_email(&self, email: &str) {
        self.inner
            .wait_for_gateway_fault_then_success_for_request_body(
                email,
                ExternalSinkFault::AbortConnectionBeforeForward,
            );
    }

    pub fn wait_for_gateway_downstream_500_then_200_for_customer_email(&self, email: &str) {
        self.inner
            .wait_for_gateway_forwarded_status_sequence_for_request_body(
                email,
                &[
                    reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                    reqwest::StatusCode::OK,
                ],
            );
    }

    pub fn runtime_shape_audit(&self) -> RuntimeShapeAudit {
        self.inner.runtime_shape_audit()
    }

    pub fn post_setup_source_audit(&self) -> PostSetupSourceAudit {
        self.inner.post_setup_source_audit()
    }

    pub fn kill_runner(&self) {
        self.inner.kill_runner();
    }

    pub fn restart_runner(&self) {
        self.inner.restart_runner();
    }

    pub fn wait_for_runner_failed_exit(&self) -> String {
        self.inner.wait_for_runner_failed_exit()
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

    pub fn fail_helper_shadow_customer_email_write(
        &self,
        customer_id: i64,
        email: &str,
    ) -> DestinationWriteFailure {
        let helper_table = self.inner.helper_table_name_for("public.customers");
        self.inner.install_destination_write_failure(
            "_cockroach_migration_tool",
            &helper_table,
            &format!(
                "NEW.id = {customer_id} AND NEW.email = '{email}'",
                email = email.replace('\'', "''"),
            ),
            "forced helper shadow write failure",
        )
    }

    pub fn fail_destination_customer_email_write(
        &self,
        customer_id: i64,
        email: &str,
    ) -> DestinationWriteFailure {
        self.inner.install_destination_write_failure(
            "public",
            "customers",
            &format!(
                "NEW.id = {customer_id} AND NEW.email = '{email}'",
                email = email.replace('\'', "''"),
            ),
            "forced destination write failure",
        )
    }

    fn build_inner(
        reconcile_interval_secs: u64,
        webhook_sink_mode: WebhookSinkMode,
    ) -> CdcE2eHarness {
        CdcE2eHarness::start_with_webhook_sink_and_runtime(
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
            DestinationRuntimeMode::HostProcess,
        )
    }
}
