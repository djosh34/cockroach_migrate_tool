use std::{collections::BTreeSet, fs, path::Path};

use serde::Deserialize;

const EXPECTED_COCKROACH_VERSION_PREFIX: &str = "v23.1.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomerLiveUpdateAudit {
    received_watermark: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScenarioOutcome {
    Harmless,
    BoundedOperatorAction,
    Defective,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappingProgressAudit {
    latest_received_resolved_watermark: Option<String>,
    latest_reconciled_resolved_watermark: Option<String>,
    last_successful_sync_watermark: Option<String>,
    last_error: Option<String>,
}

impl MappingProgressAudit {
    pub fn new(
        latest_received_resolved_watermark: Option<String>,
        latest_reconciled_resolved_watermark: Option<String>,
        last_successful_sync_watermark: Option<String>,
        last_error: Option<String>,
    ) -> Self {
        Self {
            latest_received_resolved_watermark,
            latest_reconciled_resolved_watermark,
            last_successful_sync_watermark,
            last_error,
        }
    }

    pub fn cleanly_reconciled(&self) -> bool {
        self.latest_received_resolved_watermark.is_some()
            && self.latest_received_resolved_watermark == self.latest_reconciled_resolved_watermark
            && self.last_successful_sync_watermark == self.latest_reconciled_resolved_watermark
            && self.last_error.is_none()
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateFeedAudit {
    outcome: ScenarioOutcome,
    added_changefeed_job_id: String,
    delivery_attempt_count: usize,
    helper_shadow_snapshot: String,
    helper_shadow_rows: usize,
    progress: MappingProgressAudit,
    verify_correctness: VerifyCorrectnessAudit,
}

impl DuplicateFeedAudit {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        outcome: ScenarioOutcome,
        added_changefeed_job_id: String,
        delivery_attempt_count: usize,
        helper_shadow_snapshot: String,
        helper_shadow_rows: usize,
        progress: MappingProgressAudit,
        verify_correctness: VerifyCorrectnessAudit,
    ) -> Self {
        Self {
            outcome,
            added_changefeed_job_id,
            delivery_attempt_count,
            helper_shadow_snapshot,
            helper_shadow_rows,
            progress,
            verify_correctness,
        }
    }

    pub fn assert_harmless(&self, expected_helper_snapshot: &str) {
        assert_eq!(
            self.outcome,
            ScenarioOutcome::Harmless,
            "concurrent duplicate-feed audit should classify the scenario as harmless: {:?}",
            self
        );
        assert!(
            self.delivery_attempt_count >= 2,
            "duplicate-feed audit should observe at least two deliveries for the same logical row: {:?}",
            self
        );
        assert_eq!(
            self.helper_shadow_snapshot, expected_helper_snapshot,
            "duplicate-feed audit should preserve the correct helper shadow state",
        );
        assert_eq!(
            self.helper_shadow_rows, 2,
            "duplicate-feed audit should not grow helper rows beyond the selected table cardinality",
        );
        assert!(
            self.progress.cleanly_reconciled(),
            "duplicate-feed audit should end fully reconciled without a stored error: {:?}",
            self
        );
        self.verify_correctness.assert_selected_tables_match();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecreatedFeedReplayAudit {
    outcome: ScenarioOutcome,
    original_changefeed_job_id: String,
    recreated_changefeed_job_id: String,
    delivery_attempt_count: usize,
    helper_shadow_snapshot: String,
    helper_shadow_rows: usize,
    progress: MappingProgressAudit,
    verify_correctness: VerifyCorrectnessAudit,
}

impl RecreatedFeedReplayAudit {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        outcome: ScenarioOutcome,
        original_changefeed_job_id: String,
        recreated_changefeed_job_id: String,
        delivery_attempt_count: usize,
        helper_shadow_snapshot: String,
        helper_shadow_rows: usize,
        progress: MappingProgressAudit,
        verify_correctness: VerifyCorrectnessAudit,
    ) -> Self {
        Self {
            outcome,
            original_changefeed_job_id,
            recreated_changefeed_job_id,
            delivery_attempt_count,
            helper_shadow_snapshot,
            helper_shadow_rows,
            progress,
            verify_correctness,
        }
    }

    pub fn assert_harmless(&self, expected_helper_snapshot: &str) {
        assert_eq!(
            self.outcome,
            ScenarioOutcome::Harmless,
            "recreated-feed replay audit should classify the scenario as harmless: {:?}",
            self
        );
        assert!(
            self.delivery_attempt_count >= 2,
            "recreated-feed replay audit should observe the original delivery plus replay: {:?}",
            self
        );
        assert_eq!(
            self.helper_shadow_snapshot, expected_helper_snapshot,
            "recreated-feed replay audit should preserve the correct helper shadow state",
        );
        assert_eq!(
            self.helper_shadow_rows, 2,
            "recreated-feed replay audit should not grow helper rows beyond the selected table cardinality",
        );
        assert!(
            self.progress.cleanly_reconciled(),
            "recreated-feed replay audit should end fully reconciled without a stored error: {:?}",
            self
        );
        self.verify_correctness.assert_selected_tables_match();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaMismatchAudit {
    outcome: ScenarioOutcome,
    delivery_attempt_count: usize,
    helper_shadow_snapshot: String,
    helper_shadow_rows: usize,
    progress: MappingProgressAudit,
    received_advanced_since_baseline: bool,
    reconciled_stalled_at_baseline: bool,
    runner_alive_after_failure: bool,
    runner_stderr: String,
    verify_correctness: VerifyCorrectnessAudit,
}

impl SchemaMismatchAudit {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        outcome: ScenarioOutcome,
        delivery_attempt_count: usize,
        helper_shadow_snapshot: String,
        helper_shadow_rows: usize,
        progress: MappingProgressAudit,
        received_advanced_since_baseline: bool,
        reconciled_stalled_at_baseline: bool,
        runner_alive_after_failure: bool,
        runner_stderr: String,
        verify_correctness: VerifyCorrectnessAudit,
    ) -> Self {
        Self {
            outcome,
            delivery_attempt_count,
            helper_shadow_snapshot,
            helper_shadow_rows,
            progress,
            received_advanced_since_baseline,
            reconciled_stalled_at_baseline,
            runner_alive_after_failure,
            runner_stderr,
            verify_correctness,
        }
    }

    pub fn assert_bounded_operator_action(&self, expected_helper_snapshot: &str) {
        assert_eq!(
            self.outcome,
            ScenarioOutcome::BoundedOperatorAction,
            "schema-mismatch audit should classify the scenario as a bounded operator action: {:?}",
            self
        );
        assert!(
            self.delivery_attempt_count >= 1,
            "schema-mismatch audit should observe at least one bounded ingress delivery before reconcile fails: {:?}",
            self
        );
        assert_eq!(
            self.helper_shadow_snapshot, expected_helper_snapshot,
            "schema-mismatch audit should still persist the latest helper shadow state",
        );
        assert_eq!(
            self.helper_shadow_rows, 2,
            "schema-mismatch audit should not grow helper rows while reconcile is failing",
        );
        assert!(
            self.received_advanced_since_baseline,
            "schema-mismatch audit should record that the new watermark was received",
        );
        assert!(
            self.reconciled_stalled_at_baseline,
            "schema-mismatch audit should show that reconcile stalled at the last good checkpoint",
        );
        assert!(
            self.runner_alive_after_failure,
            "schema-mismatch audit should keep the runner alive for operator intervention",
        );
        let last_error = self.progress.last_error().unwrap_or_else(|| {
            panic!(
                "schema-mismatch audit should persist an operator-visible last_error: {:?}",
                self
            )
        });
        assert!(
            last_error.contains("reconcile upsert failed for public.customers"),
            "schema-mismatch audit should persist table-specific reconcile failure context: {last_error}",
        );
        assert!(
            self.runner_stderr
                .contains("failed to apply reconcile upsert")
                && self.runner_stderr.contains("public.customers"),
            "schema-mismatch audit should emit operator-visible reconcile failure logs:\n{}",
            self.runner_stderr,
        );
        self.verify_correctness
            .assert_detects_selected_table_mismatch();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReconcileTransactionFailureAudit {
    outcome: ScenarioOutcome,
    helper_shadow_snapshot: String,
    helper_shadow_rows: usize,
    failure_progress: MappingProgressAudit,
    received_advanced_since_baseline: bool,
    reconciled_stalled_at_baseline: bool,
    runner_alive_during_failure: bool,
    runner_stderr: String,
    failed_received_watermark: String,
    failure_verify_correctness: VerifyCorrectnessAudit,
    recovery_progress: MappingProgressAudit,
    recovery_received_through_failed_watermark: bool,
    recovery_verify_correctness: VerifyCorrectnessAudit,
}

impl ReconcileTransactionFailureAudit {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        outcome: ScenarioOutcome,
        helper_shadow_snapshot: String,
        helper_shadow_rows: usize,
        failure_progress: MappingProgressAudit,
        received_advanced_since_baseline: bool,
        reconciled_stalled_at_baseline: bool,
        runner_alive_during_failure: bool,
        runner_stderr: String,
        failed_received_watermark: String,
        failure_verify_correctness: VerifyCorrectnessAudit,
        recovery_progress: MappingProgressAudit,
        recovery_received_through_failed_watermark: bool,
        recovery_verify_correctness: VerifyCorrectnessAudit,
    ) -> Self {
        Self {
            outcome,
            helper_shadow_snapshot,
            helper_shadow_rows,
            failure_progress,
            received_advanced_since_baseline,
            reconciled_stalled_at_baseline,
            runner_alive_during_failure,
            runner_stderr,
            failed_received_watermark,
            failure_verify_correctness,
            recovery_progress,
            recovery_received_through_failed_watermark,
            recovery_verify_correctness,
        }
    }

    pub fn assert_harmless_recovery(&self, expected_helper_snapshot: &str) {
        assert_eq!(
            self.outcome,
            ScenarioOutcome::Harmless,
            "reconcile transaction failure audit should classify the scenario as harmless retry-and-recovery: {:?}",
            self
        );
        assert_eq!(
            self.helper_shadow_snapshot, expected_helper_snapshot,
            "reconcile transaction failure audit should preserve the latest helper shadow state",
        );
        assert_eq!(
            self.helper_shadow_rows, 2,
            "reconcile transaction failure audit should not grow helper rows while reconcile is retrying",
        );
        assert!(
            self.received_advanced_since_baseline,
            "reconcile transaction failure audit should record that the new watermark was durably received",
        );
        assert!(
            self.reconciled_stalled_at_baseline,
            "reconcile transaction failure audit should preserve the last good reconcile checkpoint during failure",
        );
        assert!(
            self.runner_alive_during_failure,
            "reconcile transaction failure audit should keep the runner alive during transient destination write failure",
        );
        let last_error = self.failure_progress.last_error().unwrap_or_else(|| {
            panic!(
                "reconcile transaction failure audit should persist an operator-visible last_error while failing: {:?}",
                self
            )
        });
        assert!(
            last_error.contains("reconcile upsert failed for public.customers"),
            "reconcile transaction failure audit should persist table-specific reconcile failure context: {last_error}",
        );
        assert!(
            self.runner_stderr
                .contains("failed to apply reconcile upsert")
                && self.runner_stderr.contains("public.customers"),
            "reconcile transaction failure audit should emit operator-visible reconcile failure logs:\n{}",
            self.runner_stderr,
        );
        assert!(
            !self.failed_received_watermark.is_empty(),
            "reconcile transaction failure audit should capture the failed received watermark: {:?}",
            self
        );
        self.failure_verify_correctness
            .assert_detects_selected_table_mismatch();
        assert!(
            self.recovery_received_through_failed_watermark,
            "reconcile transaction failure audit should keep the received watermark monotonic across recovery",
        );
        assert!(
            self.recovery_progress.cleanly_reconciled(),
            "reconcile transaction failure audit should end fully reconciled without a stored error: {:?}",
            self
        );
        self.recovery_verify_correctness
            .assert_selected_tables_match();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyCorrectnessAudit {
    expected_tables: Vec<String>,
    job_id: String,
    status: String,
    failure_category: Option<String>,
    table_summaries: Vec<VerifyTableSummary>,
    mismatched_tables: BTreeSet<String>,
}

impl VerifyCorrectnessAudit {
    pub fn new(expected_tables: Vec<String>, response: VerifyJobResponse) -> Self {
        let result = response.result.unwrap_or_default();
        let mismatched_tables = result.mismatched_tables();
        Self {
            expected_tables,
            job_id: response.job_id,
            status: response.status,
            failure_category: response.failure.map(|failure| failure.category),
            table_summaries: result.table_summaries,
            mismatched_tables,
        }
    }

    pub fn assert_finished_with_mismatch(&self) {
        assert!(
            self.finished_with_mismatch(),
            "verify service job `{}` should finish as a mismatch failure when selected tables diverge: {:?}",
            self.job_id,
            self
        );
    }

    pub fn assert_selected_tables_match(&self) {
        assert!(
            self.selected_tables_match(),
            "verify service job `{}` should report selected-table correctness through the dedicated verify boundary: {:?}",
            self.job_id,
            self
        );
    }

    pub fn assert_detects_selected_table_mismatch(&self) {
        assert!(
            self.selected_tables_mismatch(),
            "verify service job `{}` should expose selected-table mismatches through the dedicated verify boundary: {:?}",
            self.job_id,
            self
        );
    }

    pub fn finished_successfully(&self) -> bool {
        self.status == "succeeded"
    }

    pub fn selected_tables_match(&self) -> bool {
        self.finished_successfully()
            && self.covers_expected_tables()
            && self.expected_tables.iter().all(|expected_table| {
                let table_summaries = self
                    .table_summaries
                    .iter()
                    .filter(|summary| summary.table_name() == *expected_table)
                    .collect::<Vec<_>>();
                !table_summaries.is_empty()
                    && table_summaries
                        .iter()
                        .all(|summary| summary.num_mismatch == 0)
                    && table_summaries
                        .iter()
                        .any(|summary| summary.num_verified > 0)
                    && !self.mismatched_tables.contains(expected_table)
            })
    }

    pub fn selected_tables_mismatch(&self) -> bool {
        self.finished_with_mismatch()
            && self.covers_expected_tables()
            && (self.table_summaries.iter().any(|summary| {
                self.expected_tables.contains(&summary.table_name()) && summary.num_mismatch > 0
            }) || self
                .expected_tables
                .iter()
                .any(|expected_table| self.mismatched_tables.contains(expected_table)))
    }

    fn finished_with_mismatch(&self) -> bool {
        self.status == "failed" && self.failure_category.as_deref() == Some("mismatch")
    }

    fn covers_expected_tables(&self) -> bool {
        let expected_tables = self
            .expected_tables
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let summarized_tables = self
            .table_summaries
            .iter()
            .map(VerifyTableSummary::table_name)
            .collect::<BTreeSet<_>>();

        summarized_tables == expected_tables
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct VerifyJobResponse {
    job_id: String,
    status: String,
    #[serde(default)]
    failure: Option<VerifyJobFailure>,
    #[serde(default)]
    result: Option<VerifyJobResult>,
}

impl VerifyJobResponse {
    pub(crate) fn is_running(&self) -> bool {
        self.status == "running"
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct VerifyTableSummary {
    schema: String,
    table: String,
    num_verified: usize,
    num_mismatch: usize,
}

impl VerifyTableSummary {
    fn table_name(&self) -> String {
        format!("{}.{}", self.schema, self.table)
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
struct VerifyJobResult {
    #[serde(default)]
    table_summaries: Vec<VerifyTableSummary>,
    #[serde(default)]
    findings: Vec<VerifyFinding>,
    #[serde(default)]
    mismatch_summary: VerifyMismatchSummary,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct VerifyJobFailure {
    category: String,
}

impl VerifyJobResult {
    fn mismatched_tables(&self) -> BTreeSet<String> {
        self.mismatch_summary
            .affected_tables
            .iter()
            .map(VerifyTableRef::table_name)
            .chain(self.findings.iter().map(VerifyFinding::table_name))
            .collect()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct VerifyTableRef {
    schema: String,
    table: String,
}

impl VerifyTableRef {
    fn table_name(&self) -> String {
        format!("{}.{}", self.schema, self.table)
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
struct VerifyMismatchSummary {
    #[serde(default)]
    has_mismatches: bool,
    #[serde(default)]
    affected_tables: Vec<VerifyTableRef>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct VerifyFinding {
    kind: String,
    schema: String,
    table: String,
}

impl VerifyFinding {
    fn table_name(&self) -> String {
        format!("{}.{}", self.schema, self.table)
    }
}

impl CustomerLiveUpdateAudit {
    pub fn new(received_watermark: String) -> Self {
        Self { received_watermark }
    }

    pub fn received_watermark(&self) -> &str {
        &self.received_watermark
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceCommandAudit {
    commands: Vec<RecordedSourceCommand>,
    bootstrap_command_count: usize,
}

impl SourceCommandAudit {
    pub fn from_cockroach_log(log_path: &Path, bootstrap_command_count: usize) -> Self {
        let commands = parse_logged_commands(log_path, "cockroach command log")
            .into_iter()
            .enumerate()
            .map(|(index, args)| RecordedSourceCommand {
                phase: if index < bootstrap_command_count {
                    SourceCommandPhase::Bootstrap
                } else {
                    SourceCommandPhase::PostSetup
                },
                sql: cockroach_sql_argument(&args),
            })
            .collect::<Vec<_>>();

        assert!(
            commands.len() >= bootstrap_command_count,
            "cockroach command log should contain at least {bootstrap_command_count} audited source commands: {commands:?}",
        );

        Self {
            commands,
            bootstrap_command_count,
        }
    }

    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    pub fn bootstrap(&self) -> BootstrapSourceAudit {
        BootstrapSourceAudit {
            commands: self.commands[..self.bootstrap_command_count].to_vec(),
        }
    }

    pub fn post_setup(&self) -> PostSetupSourceAudit {
        let commands = self.commands[self.bootstrap_command_count..].to_vec();
        PostSetupSourceAudit { commands }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootstrapSourceAudit {
    commands: Vec<RecordedSourceCommand>,
}

impl BootstrapSourceAudit {
    pub fn assert_creates_changefeed_for(&self, selected_tables: &[&str]) {
        let changefeed_command = self
            .commands
            .iter()
            .find(|command| command.sql.contains("CREATE CHANGEFEED"))
            .unwrap_or_else(|| {
                panic!(
                    "bootstrap source commands should include CREATE CHANGEFEED: {:?}",
                    self.commands
                )
            });

        assert_eq!(
            changefeed_command.phase,
            SourceCommandPhase::Bootstrap,
            "initial CREATE CHANGEFEED must belong to bootstrap commands: {:?}",
            self.commands,
        );

        for table in selected_tables {
            assert!(
                changefeed_command.sql.contains(table),
                "initial CREATE CHANGEFEED should include selected table `{table}`: {:?}",
                changefeed_command,
            );
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostSetupSourceAudit {
    commands: Vec<RecordedSourceCommand>,
}

impl PostSetupSourceAudit {
    pub fn assert_honest_workload_only(&self, expected_commands: usize) {
        self.assert_command_count(expected_commands);
        self.assert_only_workload_dml();
    }

    pub fn assert_command_count(&self, expected: usize) {
        assert_eq!(
            self.commands.len(),
            expected,
            "unexpected post-setup source command count: {:?}",
            self.commands,
        );
    }

    pub fn assert_only_workload_dml(&self) {
        let forbidden_fragments = [
            "SET CLUSTER SETTING",
            "SELECT cluster_logical_timestamp()",
            "CREATE CHANGEFEED",
            "_cockroach_migration_tool",
            "information_schema.",
            "crdb_internal.",
            "system.",
        ];
        let allowed_prefixes = ["INSERT ", "UPDATE ", "DELETE "];

        for command in &self.commands {
            assert_eq!(
                command.phase,
                SourceCommandPhase::PostSetup,
                "post-setup audit should only contain post-setup commands: {:?}",
                self.commands,
            );
            let statements = split_sql_statements(&command.sql);
            assert!(
                !statements.is_empty(),
                "post-setup source command should contain at least one SQL statement: {:?}",
                command,
            );

            for (index, statement) in statements.iter().enumerate() {
                let normalized = collapse_sql_whitespace(statement).to_uppercase();
                if index == 0 && normalized.starts_with("USE ") {
                    continue;
                }
                assert!(
                    allowed_prefixes
                        .iter()
                        .any(|prefix| normalized.starts_with(prefix)),
                    "post-setup source commands must stay workload DML-only: {:?}",
                    command,
                );
                for fragment in forbidden_fragments {
                    assert!(
                        !normalized.contains(fragment),
                        "post-setup source command must not re-run bootstrap/admin/helper SQL `{fragment}`: {:?}",
                        command,
                    );
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceCommandPhase {
    Bootstrap,
    PostSetup,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RecordedSourceCommand {
    phase: SourceCommandPhase,
    sql: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeShapeAudit {
    destination_runtime: DestinationRuntimeAudit,
    changefeed_sink_base_url: String,
    cockroach: CockroachRuntimeAudit,
    destination_role: DestinationRoleAudit,
}

impl RuntimeShapeAudit {
    pub fn new(
        destination_runtime: DestinationRuntimeAudit,
        changefeed_sink_base_url: String,
        cockroach: CockroachRuntimeAudit,
        destination_role: DestinationRoleAudit,
    ) -> Self {
        Self {
            destination_runtime,
            changefeed_sink_base_url,
            cockroach,
            destination_role,
        }
    }

    pub fn assert_honest_default_runtime_shape(&self) {
        assert_eq!(
            self.destination_runtime.mode,
            DestinationRuntimeMode::HostProcess,
            "the honest default E2E path must run the destination runtime directly as the local runner process",
        );
        assert_eq!(
            self.destination_runtime.container_count, 0,
            "the honest default E2E path must not add an extra destination runner container",
        );
        assert_eq!(
            self.destination_runtime.destination_connection_host, "127.0.0.1",
            "the honest default E2E path must apply into PostgreSQL through the host-process connection contract",
        );
        assert!(
            self.destination_runtime.destination_connection_port > 0,
            "the honest default E2E path must expose a usable PostgreSQL port: {:?}",
            self.destination_runtime,
        );
        assert!(
            self.destination_runtime
                .healthcheck_url
                .starts_with("https://"),
            "runner health must be served over HTTPS: {:?}",
            self.destination_runtime,
        );
        assert!(
            self.changefeed_sink_base_url.starts_with("https://"),
            "changefeed sink must target the destination runtime over HTTPS: {}",
            self.changefeed_sink_base_url,
        );
        assert!(
            self.cockroach
                .image
                .starts_with(EXPECTED_COCKROACH_VERSION_PREFIX),
            "the honest default E2E path must use CockroachDB 23.1 from the Nix flake, got {}",
            self.cockroach.image,
        );
        assert!(
            !self.destination_role.is_superuser,
            "the destination runtime role must not be superuser: {:?}",
            self.destination_role,
        );
        if let Some(postgres_apply_client_addr) = self
            .destination_runtime
            .postgres_apply_client_addr
            .as_deref()
        {
            assert_eq!(
                Some("127.0.0.1"),
                Some(postgres_apply_client_addr),
                "when PostgreSQL exposes a live runtime session, it must originate from the local runner process",
            );
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestinationRuntimeAudit {
    pub(crate) mode: DestinationRuntimeMode,
    pub(crate) container_count: usize,
    pub(crate) healthcheck_url: String,
    pub(crate) destination_connection_host: String,
    pub(crate) destination_connection_port: u16,
    pub(crate) runner_container_ip: Option<String>,
    pub(crate) postgres_apply_client_addr: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationRuntimeMode {
    HostProcess,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CockroachRuntimeAudit {
    image: String,
}

impl CockroachRuntimeAudit {
    pub fn new(image: String) -> Self {
        Self { image }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestinationRoleAudit {
    role_name: String,
    is_superuser: bool,
}

impl DestinationRoleAudit {
    pub fn new(role_name: String, is_superuser: bool) -> Self {
        Self {
            role_name,
            is_superuser,
        }
    }
}

fn parse_logged_commands(log_path: &Path, description: &str) -> Vec<Vec<String>> {
    let log = fs::read_to_string(log_path).unwrap_or_else(|error| {
        panic!(
            "{description} log `{}` should be readable: {error}",
            log_path.display()
        )
    });
    parse_logged_commands_from_text(&log, description)
}

fn parse_logged_commands_from_text(log: &str, description: &str) -> Vec<Vec<String>> {
    let mut commands = Vec::new();
    let mut current = Vec::new();

    for line in log.lines() {
        if line == "END" {
            if !current.is_empty() {
                commands.push(std::mem::take(&mut current));
            }
            continue;
        }

        if let Some(argument) = line.strip_prefix("ARG\t") {
            current.push(argument.to_owned());
            continue;
        }
        if let Some(argument) = line.strip_prefix("ARG_ESC\t") {
            current.push(unescape_logged_argument(argument));
            continue;
        }
        panic!("{description} log line should start with `ARG\\t` or `ARG_ESC\\t`: {line}");
    }

    if !current.is_empty() {
        commands.push(current);
    }

    commands
}

fn cockroach_sql_argument(args: &[String]) -> String {
    assert!(
        args.first().map(String::as_str) == Some("sql"),
        "cockroach command should be invoked with `sql`: {args:?}",
    );
    optional_flag_value(args, "-e")
        .or_else(|| optional_flag_value(args, "--execute"))
        .unwrap_or_else(|| panic!("cockroach command should include `-e` or `--execute`: {args:?}"))
}

fn optional_flag_value(args: &[String], flag: &str) -> Option<String> {
    let position = args.iter().position(|arg| arg == flag)?;
    Some(
        args.get(position + 1)
            .unwrap_or_else(|| panic!("command should include a value after `{flag}`: {args:?}"))
            .clone(),
    )
}

fn split_sql_statements(sql: &str) -> Vec<&str> {
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .collect()
}

fn collapse_sql_whitespace(sql: &str) -> String {
    sql.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn unescape_logged_argument(argument: &str) -> String {
    let mut unescaped = String::new();
    let mut chars = argument.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            unescaped.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => unescaped.push('\n'),
            Some('t') => unescaped.push('\t'),
            Some('\\') => unescaped.push('\\'),
            Some(other) => {
                unescaped.push('\\');
                unescaped.push(other);
            }
            None => unescaped.push('\\'),
        }
    }
    unescaped
}
