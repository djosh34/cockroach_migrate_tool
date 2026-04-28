use std::time::Duration;

use crate::e2e_harness::{
    CdcE2eHarness, CdcE2eHarnessConfig, ChangefeedInitialScanMode, DestinationTableLock,
    DestinationWriteFailure, MappingTrackingProgress, WebhookSinkMode,
};
use crate::e2e_integrity::{
    CustomerLiveUpdateAudit, DestinationRuntimeMode, DuplicateFeedAudit, MappingProgressAudit,
    PostSetupSourceAudit, ReconcileTransactionFailureAudit, RecreatedFeedReplayAudit,
    RuntimeShapeAudit, ScenarioOutcome, SchemaMismatchAudit, VerifyCorrectnessAudit,
};
use crate::verify_service_harness_support::VerifyServiceHarness;
use crate::webhook_chaos_gateway::ExternalSinkFault;

const DEFAULT_SOURCE_SCHEMA_SQL: &str = r#"
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

const DEFAULT_DESTINATION_SCHEMA_SQL: &str = r#"
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

const CONCURRENT_DUPLICATE_FEED_EMAIL: &str = "alice+dual-feed@example.com";
const RECREATED_FEED_REPLAY_EMAIL: &str = "alice+replay@example.com";
const SCHEMA_MISMATCH_EMAIL: &str = "alice+schema-mismatch@example.com";

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

    pub fn assert_helper_shadow_customers_stable(&self, expected: &str, duration: Duration) {
        self.inner.assert_destination_query_stable(
            HELPER_SHADOW_CUSTOMERS_SNAPSHOT_SQL,
            expected,
            "helper shadow customers",
            duration,
        );
    }

    pub fn wait_for_selected_tables_to_match_via_verify_service(
        &self,
        verify_service: &VerifyServiceHarness,
    ) -> VerifyCorrectnessAudit {
        let audit = self.inner.wait_for_selected_tables_to_match_via_verify_service(
            verify_service,
            "default selected tables should converge through the verify service",
        );
        self.wait_for_customer_tracking_progress(
            "default selected tables should not be treated as converged until customer tracking is fully reconciled",
            |progress| {
                progress.stream.latest_received_resolved_watermark
                    == progress.stream.latest_reconciled_resolved_watermark
                    && progress.table.last_successful_sync_watermark
                        == progress.stream.latest_reconciled_resolved_watermark
                    && progress.table.last_error.is_none()
            },
        );
        audit
    }

    pub fn wait_for_selected_tables_to_mismatch_via_verify_service(
        &self,
        verify_service: &VerifyServiceHarness,
    ) -> VerifyCorrectnessAudit {
        self.inner.wait_for_selected_tables_to_mismatch_via_verify_service(
            verify_service,
            "default selected tables should expose divergence through the verify service",
        )
    }

    pub fn assert_selected_tables_match_via_verify_service_stable(
        &self,
        verify_service: &VerifyServiceHarness,
        duration: Duration,
    ) {
        self.inner.assert_selected_tables_match_via_verify_service_stable(
            verify_service,
            "default selected tables should remain matched through the verify service",
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
        verify_service: &VerifyServiceHarness,
        audit: &CustomerLiveUpdateAudit,
    ) -> MappingTrackingProgress {
        self.wait_for_selected_tables_to_match_via_verify_service(verify_service);
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

    pub fn audit_concurrent_duplicate_customer_feeds(
        &self,
        verify_service: &VerifyServiceHarness,
    ) -> DuplicateFeedAudit {
        self.wait_for_selected_tables_to_match_via_verify_service(verify_service)
            .assert_selected_tables_match();
        let added_changefeed_job_id = self
            .inner
            .create_additional_changefeed_from_current_cursor(ChangefeedInitialScanMode::No);
        self.update_source_customer_email(1, CONCURRENT_DUPLICATE_FEED_EMAIL);
        let delivery_attempt_count = self
            .inner
            .wait_for_gateway_attempt_count_for_request_body(CONCURRENT_DUPLICATE_FEED_EMAIL, 2);
        let observed_source_job_ids = self
            .inner
            .wait_for_gateway_source_job_ids_for_request_body(CONCURRENT_DUPLICATE_FEED_EMAIL, 2);
        let expected_helper_snapshot =
            format!("1:{CONCURRENT_DUPLICATE_FEED_EMAIL},2:bob@example.com");
        self.wait_for_helper_shadow_customers(&expected_helper_snapshot);
        self.assert_helper_shadow_customers(2);
        let verify_correctness =
            self.wait_for_selected_tables_to_match_via_verify_service(verify_service);
        self.assert_helper_shadow_customers_stable(
            &expected_helper_snapshot,
            Duration::from_secs(3),
        );
        let progress = self.mapping_progress_audit();
        let outcome = if delivery_attempt_count >= 2
            && observed_source_job_ids.contains(&added_changefeed_job_id)
            && observed_source_job_ids.len() >= 2
            && progress.cleanly_reconciled()
            && verify_correctness.selected_tables_match()
        {
            ScenarioOutcome::Harmless
        } else {
            ScenarioOutcome::Defective
        };

        DuplicateFeedAudit::new(
            outcome,
            added_changefeed_job_id,
            observed_source_job_ids,
            delivery_attempt_count,
            expected_helper_snapshot,
            self.inner.helper_table_row_count("public.customers"),
            progress,
            verify_correctness,
        )
    }

    pub fn audit_recreated_customer_feed_replay(
        &self,
        verify_service: &VerifyServiceHarness,
    ) -> RecreatedFeedReplayAudit {
        self.wait_for_selected_tables_to_match_via_verify_service(verify_service)
            .assert_selected_tables_match();
        self.update_source_customer_email(1, RECREATED_FEED_REPLAY_EMAIL);
        self.wait_for_helper_shadow_customers(&format!(
            "1:{RECREATED_FEED_REPLAY_EMAIL},2:bob@example.com"
        ));
        self.wait_for_selected_tables_to_match_via_verify_service(verify_service)
            .assert_selected_tables_match();
        let original_changefeed_job_id = self
            .inner
            .wait_for_gateway_source_job_ids_for_request_body(RECREATED_FEED_REPLAY_EMAIL, 1)
            .into_iter()
            .next()
            .expect("original changefeed delivery should expose a source job id");
        self.inner
            .cancel_changefeed_job(&original_changefeed_job_id);
        let recreated_changefeed_job_id = self
            .inner
            .create_additional_changefeed_from_current_cursor(ChangefeedInitialScanMode::Yes);
        let delivery_attempt_count = self
            .inner
            .wait_for_gateway_attempt_count_for_request_body(RECREATED_FEED_REPLAY_EMAIL, 2);
        let observed_source_job_ids = self
            .inner
            .wait_for_gateway_source_job_ids_for_request_body(RECREATED_FEED_REPLAY_EMAIL, 2);
        let expected_helper_snapshot = format!("1:{RECREATED_FEED_REPLAY_EMAIL},2:bob@example.com");
        self.wait_for_helper_shadow_customers(&expected_helper_snapshot);
        self.assert_helper_shadow_customers(2);
        let verify_correctness =
            self.wait_for_selected_tables_to_match_via_verify_service(verify_service);
        self.assert_helper_shadow_customers_stable(
            &expected_helper_snapshot,
            Duration::from_secs(3),
        );
        let progress = self.mapping_progress_audit();
        let outcome = if delivery_attempt_count >= 2
            && observed_source_job_ids.contains(&original_changefeed_job_id)
            && observed_source_job_ids.contains(&recreated_changefeed_job_id)
            && progress.cleanly_reconciled()
            && verify_correctness.selected_tables_match()
        {
            ScenarioOutcome::Harmless
        } else {
            ScenarioOutcome::Defective
        };

        RecreatedFeedReplayAudit::new(
            outcome,
            original_changefeed_job_id,
            recreated_changefeed_job_id,
            observed_source_job_ids,
            delivery_attempt_count,
            expected_helper_snapshot,
            self.inner.helper_table_row_count("public.customers"),
            progress,
            verify_correctness,
        )
    }

    pub fn audit_customer_schema_mismatch(
        &self,
        verify_service: &VerifyServiceHarness,
    ) -> SchemaMismatchAudit {
        self.wait_for_selected_tables_to_match_via_verify_service(verify_service)
            .assert_selected_tables_match();
        let baseline = self.customer_tracking_progress();
        self.introduce_customer_email_schema_mismatch();
        self.update_source_customer_email(1, SCHEMA_MISMATCH_EMAIL);
        self.inner
            .wait_for_gateway_forwarded_status_sequence_for_request_body(
                SCHEMA_MISMATCH_EMAIL,
                &[reqwest::StatusCode::OK],
            );
        let delivery_attempt_count = self
            .inner
            .wait_for_gateway_attempt_count_to_stabilize_for_request_body(
                SCHEMA_MISMATCH_EMAIL,
                1,
                Duration::from_secs(3),
            );
        let expected_helper_snapshot = format!("1:{SCHEMA_MISMATCH_EMAIL},2:bob@example.com");
        self.wait_for_helper_shadow_customers(&expected_helper_snapshot);
        let verify_correctness =
            self.wait_for_selected_tables_to_mismatch_via_verify_service(verify_service);
        let failure_progress = self.wait_for_customer_tracking_progress(
            "schema mismatch should leave the new watermark received, the last good reconcile checkpoint intact, and a persisted error for operators",
            |progress| {
                progress
                    .stream
                    .received_has_advanced_since(&baseline.stream)
                    && progress.stream.latest_reconciled_resolved_watermark
                        == baseline.stream.latest_reconciled_resolved_watermark
                    && progress.table.last_successful_sync_watermark
                        == baseline.table.last_successful_sync_watermark
                    && progress.table.last_error.is_some()
            },
        );
        self.assert_runner_alive();
        let progress = mapping_progress_audit_from(&failure_progress);
        let outcome = if delivery_attempt_count >= 1
            && verify_correctness.selected_tables_mismatch()
            && progress.last_error().is_some()
            && failure_progress
                .stream
                .received_has_advanced_since(&baseline.stream)
            && failure_progress.stream.latest_reconciled_resolved_watermark
                == baseline.stream.latest_reconciled_resolved_watermark
        {
            ScenarioOutcome::BoundedOperatorAction
        } else {
            ScenarioOutcome::Defective
        };

        SchemaMismatchAudit::new(
            outcome,
            delivery_attempt_count,
            expected_helper_snapshot,
            self.inner.helper_table_row_count("public.customers"),
            progress,
            failure_progress
                .stream
                .received_has_advanced_since(&baseline.stream),
            failure_progress.stream.latest_reconciled_resolved_watermark
                == baseline.stream.latest_reconciled_resolved_watermark,
            true,
            self.inner.runner_stderr_snapshot(),
            verify_correctness,
        )
    }

    pub fn audit_reconcile_transaction_failure_recovery(
        &self,
        verify_service: &VerifyServiceHarness,
    ) -> ReconcileTransactionFailureAudit {
        const RECONCILE_FAILURE_EMAIL: &str = "alice+reconcile-failure@example.com";

        self.wait_for_selected_tables_to_match_via_verify_service(verify_service)
            .assert_selected_tables_match();
        let baseline = self.customer_tracking_progress();
        let destination_failure =
            self.fail_destination_customer_email_write(1, RECONCILE_FAILURE_EMAIL);
        self.update_source_customer_email(1, RECONCILE_FAILURE_EMAIL);
        let expected_helper_snapshot = format!("1:{RECONCILE_FAILURE_EMAIL},2:bob@example.com");
        self.wait_for_helper_shadow_customers(&expected_helper_snapshot);
        let failure_verify_correctness =
            self.wait_for_selected_tables_to_mismatch_via_verify_service(verify_service);
        let failure_progress = self.wait_for_customer_tracking_progress(
            "reconcile transaction failure should keep the runner alive while persisting the received watermark, the last good checkpoint, and a last_error",
            |progress| {
                progress
                    .stream
                    .received_has_advanced_since(&baseline.stream)
                    && progress.stream.latest_reconciled_resolved_watermark
                        == baseline.stream.latest_reconciled_resolved_watermark
                    && progress.table.last_successful_sync_watermark
                        == baseline.table.last_successful_sync_watermark
                    && progress.table.last_error.is_some()
            },
        );
        self.assert_runner_alive();
        let failed_received_watermark = failure_progress
            .stream
            .latest_received_resolved_watermark
            .clone()
            .expect("failed reconcile should persist a received watermark");
        let failure_progress_audit = mapping_progress_audit_from(&failure_progress);
        let runner_stderr = self.inner.runner_stderr_snapshot();

        drop(destination_failure);

        let recovery_verify_correctness =
            self.wait_for_selected_tables_to_match_via_verify_service(verify_service);
        let recovery_progress = self.wait_for_customer_tracking_progress(
            "runner should reconcile the failed watermark in place and clear the stored error after destination writes recover",
            |progress| {
                progress.has_reconciled_through(failed_received_watermark.as_str())
                    && progress.table.last_error.is_none()
            },
        );
        let recovery_progress_audit = mapping_progress_audit_from(&recovery_progress);
        let outcome = if failure_verify_correctness.selected_tables_mismatch()
            && failure_progress
                .stream
                .received_has_advanced_since(&baseline.stream)
            && failure_progress.stream.latest_reconciled_resolved_watermark
                == baseline.stream.latest_reconciled_resolved_watermark
            && failure_progress.table.last_successful_sync_watermark
                == baseline.table.last_successful_sync_watermark
            && failure_progress.table.last_error.is_some()
            && recovery_progress.has_reconciled_through(failed_received_watermark.as_str())
            && recovery_progress
                .stream
                .has_received_through(failed_received_watermark.as_str())
            && recovery_progress.table.last_error.is_none()
            && recovery_verify_correctness.selected_tables_match()
        {
            ScenarioOutcome::Harmless
        } else {
            ScenarioOutcome::Defective
        };

        ReconcileTransactionFailureAudit::new(
            outcome,
            expected_helper_snapshot,
            self.inner.helper_table_row_count("public.customers"),
            failure_progress_audit,
            failure_progress
                .stream
                .received_has_advanced_since(&baseline.stream),
            failure_progress.stream.latest_reconciled_resolved_watermark
                == baseline.stream.latest_reconciled_resolved_watermark,
            true,
            runner_stderr,
            failed_received_watermark.clone(),
            failure_verify_correctness,
            recovery_progress_audit,
            recovery_progress
                .stream
                .has_received_through(failed_received_watermark.as_str()),
            recovery_verify_correctness,
        )
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

    fn introduce_customer_email_schema_mismatch(&self) {
        self.inner.query_destination(
            "ALTER TABLE public.customers
             ALTER COLUMN email TYPE bigint
             USING 0;",
        );
    }

    fn mapping_progress_audit(&self) -> MappingProgressAudit {
        mapping_progress_audit_from(&self.customer_tracking_progress())
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
                source_schema_sql: DEFAULT_SOURCE_SCHEMA_SQL,
                destination_schema_sql: DEFAULT_DESTINATION_SCHEMA_SQL,
            },
            webhook_sink_mode,
            DestinationRuntimeMode::HostProcess,
        )
    }
}

fn mapping_progress_audit_from(progress: &MappingTrackingProgress) -> MappingProgressAudit {
    MappingProgressAudit::new(
        progress.stream.latest_received_resolved_watermark.clone(),
        progress.stream.latest_reconciled_resolved_watermark.clone(),
        progress.table.last_successful_sync_watermark.clone(),
        progress.table.last_error.clone(),
    )
}
