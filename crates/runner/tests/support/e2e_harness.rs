use std::{
    cell::RefCell,
    fs::{self},
    io,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Mutex, MutexGuard, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use clap::Parser as _;
use reqwest::{Certificate, blocking::Client};
use serde::Deserialize;
use tempfile::TempDir;

mod destination_lock;
mod destination_write_failure;
mod runner_container_process;
mod runner_docker_contract;
mod runner_process;

pub(crate) use destination_lock::DestinationTableLock;
pub(crate) use destination_write_failure::DestinationWriteFailure;
use destination_write_failure::DestinationWriteFailureSpec;
use runner_container_process::RunnerContainerProcess;
use runner_process::RunnerProcess;

use crate::e2e_integrity::{
    CockroachRuntimeAudit, DestinationRoleAudit, DestinationRuntimeAudit, DestinationRuntimeMode,
    PostSetupSourceAudit, RuntimeShapeAudit, SourceBootstrapAudit, SourceCommandAudit,
    VerifyCorrectnessAudit,
};
use crate::verify_image_harness_support::{VerifyImageHarness, VerifyImageRun};
use crate::webhook_chaos_gateway::{ExternalSinkFault, WebhookChaosGateway};

const COCKROACH_IMAGE: &str = "cockroachdb/cockroach:v26.1.2";
const POSTGRES_IMAGE: &str = "postgres:16";
#[derive(Clone, Copy)]
pub enum WebhookSinkMode {
    DirectRunner,
    ObservableChaosGateway,
}

pub struct CdcE2eHarnessConfig<'a> {
    pub mapping_id: &'a str,
    pub source_database: &'a str,
    pub destination_database: &'a str,
    pub destination_user: &'a str,
    pub destination_password: &'a str,
    pub reconcile_interval_secs: u64,
    pub selected_tables: &'a [&'a str],
    pub source_setup_sql: &'a str,
    pub destination_setup_sql: &'a str,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct StreamTrackingProgress {
    pub latest_received_resolved_watermark: Option<String>,
    pub latest_reconciled_resolved_watermark: Option<String>,
}

impl StreamTrackingProgress {
    pub fn received_has_advanced_since(&self, earlier: &Self) -> bool {
        watermark_has_advanced(
            self.latest_received_resolved_watermark.as_deref(),
            earlier.latest_received_resolved_watermark.as_deref(),
        )
    }

    pub fn has_received_through(&self, watermark: &str) -> bool {
        watermark_at_least(
            self.latest_received_resolved_watermark.as_deref(),
            watermark,
        )
    }

    pub fn has_reconciled_through(&self, watermark: &str) -> bool {
        watermark_at_least(
            self.latest_reconciled_resolved_watermark.as_deref(),
            watermark,
        )
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct TableTrackingProgress {
    pub source_table_name: String,
    pub helper_table_name: String,
    pub last_successful_sync_watermark: Option<String>,
    pub last_error: Option<String>,
}

impl TableTrackingProgress {
    pub fn has_synced_through(&self, watermark: &str) -> bool {
        watermark_at_least(self.last_successful_sync_watermark.as_deref(), watermark)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappingTrackingProgress {
    pub stream: StreamTrackingProgress,
    pub table: TableTrackingProgress,
}

impl MappingTrackingProgress {
    pub fn has_reconciled_through(&self, watermark: &str) -> bool {
        self.stream.has_received_through(watermark)
            && self.stream.has_reconciled_through(watermark)
            && self.table.has_synced_through(watermark)
    }
}

struct OwnedHarnessConfig {
    mapping_id: String,
    source_database: String,
    destination_database: String,
    destination_user: String,
    destination_password: String,
    reconcile_interval_secs: u64,
    selected_tables: Vec<String>,
    source_setup_sql: String,
    destination_setup_sql: String,
}

impl<'a> From<CdcE2eHarnessConfig<'a>> for OwnedHarnessConfig {
    fn from(config: CdcE2eHarnessConfig<'a>) -> Self {
        Self {
            mapping_id: config.mapping_id.to_owned(),
            source_database: config.source_database.to_owned(),
            destination_database: config.destination_database.to_owned(),
            destination_user: config.destination_user.to_owned(),
            destination_password: config.destination_password.to_owned(),
            reconcile_interval_secs: config.reconcile_interval_secs,
            selected_tables: config
                .selected_tables
                .iter()
                .map(|table| (*table).to_owned())
                .collect(),
            source_setup_sql: config.source_setup_sql.to_owned(),
            destination_setup_sql: config.destination_setup_sql.to_owned(),
        }
    }
}

enum RunnerRuntime {
    Host(RunnerProcess),
    Container(RunnerContainerProcess),
}

pub struct CdcE2eHarness {
    _docker_test_guard: MutexGuard<'static, ()>,
    docker: DockerEnvironment,
    config: OwnedHarnessConfig,
    temp_dir: TempDir,
    runner_port: u16,
    destination_runtime_mode: DestinationRuntimeMode,
    webhook_sink_base_url: String,
    webhook_chaos_gateway: Option<WebhookChaosGateway>,
    runner_config_path: PathBuf,
    source_bootstrap_config_path: PathBuf,
    wrapper_bin_dir: PathBuf,
    cockroach_wrapper_log_path: PathBuf,
    runner_stdout_path: PathBuf,
    runner_stderr_path: PathBuf,
    bootstrap_source_command_count: RefCell<Option<usize>>,
    runner_process: RefCell<Option<RunnerRuntime>>,
}

impl CdcE2eHarness {
    pub fn start(config: CdcE2eHarnessConfig<'_>) -> Self {
        Self::start_with_webhook_sink(config, WebhookSinkMode::DirectRunner)
    }

    pub fn start_with_webhook_sink(
        config: CdcE2eHarnessConfig<'_>,
        webhook_sink_mode: WebhookSinkMode,
    ) -> Self {
        Self::start_with_webhook_sink_and_runtime(
            config,
            webhook_sink_mode,
            DestinationRuntimeMode::HostProcess,
        )
    }

    pub fn start_with_webhook_sink_and_runtime(
        config: CdcE2eHarnessConfig<'_>,
        webhook_sink_mode: WebhookSinkMode,
        destination_runtime_mode: DestinationRuntimeMode,
    ) -> Self {
        let config = OwnedHarnessConfig::from(config);
        let docker_test_guard = lock_e2e_docker_resources();
        let docker = DockerEnvironment::new();
        docker.create_network();
        docker.start_cockroach();
        docker.start_postgres();
        docker.wait_for_cockroach();
        docker.wait_for_postgres();
        docker.prepare_source_schema_and_seed(&config.source_setup_sql);
        docker.prepare_destination_database(
            &config.destination_database,
            &config.destination_user,
            &config.destination_password,
            &config.destination_setup_sql,
        );

        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let runner_port = pick_unused_port();
        let wrapper_bin_dir = temp_dir.path().join("bin");
        fs::create_dir_all(&wrapper_bin_dir).expect("wrapper bin dir should be created");

        let harness = Self {
            _docker_test_guard: docker_test_guard,
            docker,
            config,
            temp_dir,
            runner_port,
            destination_runtime_mode,
            webhook_sink_base_url: String::new(),
            webhook_chaos_gateway: None,
            runner_config_path: PathBuf::new(),
            source_bootstrap_config_path: PathBuf::new(),
            wrapper_bin_dir,
            cockroach_wrapper_log_path: PathBuf::new(),
            runner_stdout_path: PathBuf::new(),
            runner_stderr_path: PathBuf::new(),
            bootstrap_source_command_count: RefCell::new(None),
            runner_process: RefCell::new(None),
        };
        harness.materialize(webhook_sink_mode)
    }

    pub fn bootstrap_migration(&self) {
        self.start_runner_process();
        wait_for_runner_health(
            &https_client(&investigation_ca_cert_path()),
            self.runner_port,
            || self.runner_logs(),
        );
        let source_bootstrap_sql = self.render_source_bootstrap_sql();
        self.apply_source_bootstrap_sql(&source_bootstrap_sql);
        let bootstrap_command_count = self.read_source_command_count();
        *self.bootstrap_source_command_count.borrow_mut() = Some(bootstrap_command_count);
    }

    pub fn kill_runner(&self) {
        let mut runner_process = self.runner_process.borrow_mut();
        let process = runner_process
            .take()
            .expect("runner process should exist before it can be killed");
        match process {
            RunnerRuntime::Host(mut process) => process.kill(),
            RunnerRuntime::Container(process) => process.kill(),
        }
    }

    pub fn restart_runner(&self) {
        if self.runner_process.borrow().is_some() {
            panic!("runner process should not already be running during restart");
        }
        self.start_runner_process();
        wait_for_runner_health(
            &https_client(&investigation_ca_cert_path()),
            self.runner_port,
            || self.runner_logs(),
        );
    }

    pub fn wait_for_destination_query(&self, sql: &str, expected: &str, description: &str) {
        for _ in 0..120 {
            self.assert_runner_process_alive();
            let actual = self.query_destination(sql);
            if actual.trim() == expected {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "{description} did not converge to `{expected}`\nactual={}\nrunner stderr:\n{}",
            self.query_destination(sql).trim(),
            self.runner_diagnostics(),
        );
    }

    pub fn assert_destination_query_stable(
        &self,
        sql: &str,
        expected: &str,
        description: &str,
        duration: Duration,
    ) {
        let deadline = Instant::now() + duration;
        loop {
            self.assert_runner_process_alive();
            let actual = self.query_destination(sql);
            assert_eq!(
                actual.trim(),
                expected,
                "{description} changed unexpectedly while it should remain stable\nrunner stderr:\n{}",
                self.runner_diagnostics(),
            );
            if Instant::now() >= deadline {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    pub fn wait_for_helper_table_row_counts(&self, expectations: &[(&str, usize)]) {
        for _ in 0..120 {
            self.assert_runner_process_alive();
            if expectations
                .iter()
                .all(|(table, expected_rows)| self.helper_table_row_count(table) == *expected_rows)
            {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        let actual = expectations
            .iter()
            .map(|(table, expected_rows)| {
                format!(
                    "{table}: expected={expected_rows} actual={}",
                    self.helper_table_row_count(table)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        panic!(
            "helper shadow tables did not converge to expected row counts: {actual}\nhelper tables={}\nrunner stderr:\n{}",
            self.helper_tables().trim(),
            self.runner_diagnostics(),
        );
    }

    pub fn wait_for_helper_tables(&self, expected: &str, description: &str) {
        for _ in 0..120 {
            self.assert_runner_process_alive();
            let actual = self.helper_tables();
            if actual.trim() == expected {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "{description} did not converge to `{expected}`\nactual={}\nrunner stderr:\n{}",
            self.helper_tables().trim(),
            self.runner_diagnostics(),
        );
    }

    pub fn assert_explicit_source_bootstrap_commands(&self) {
        let expected_tables = self
            .config
            .selected_tables
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        self.source_command_audit()
            .assert_explicit_bootstrap_commands(&self.config.source_database, &expected_tables);
    }

    pub fn runtime_shape_audit(&self) -> RuntimeShapeAudit {
        let apply_client_addr = self.destination_runtime_client_addr();
        let destination_runtime = {
            let runner_process = self.runner_process.borrow();
            match runner_process.as_ref() {
                Some(RunnerRuntime::Host(_)) => DestinationRuntimeAudit {
                    mode: DestinationRuntimeMode::HostProcess,
                    container_count: 0,
                    runner_entrypoint_json: None,
                    healthcheck_url: format!("https://localhost:{}/healthz", self.runner_port),
                    destination_connection_host: self
                        .destination_runtime_postgres_host()
                        .to_owned(),
                    destination_connection_port: self.destination_runtime_postgres_port(),
                    runner_container_ip: None,
                    postgres_apply_client_addr: apply_client_addr.clone(),
                },
                Some(RunnerRuntime::Container(process)) => DestinationRuntimeAudit {
                    mode: DestinationRuntimeMode::SingleContainer,
                    container_count: 1,
                    runner_entrypoint_json: Some(process.image_entrypoint_json()),
                    healthcheck_url: format!("https://localhost:{}/healthz", self.runner_port),
                    destination_connection_host: self
                        .destination_runtime_postgres_host()
                        .to_owned(),
                    destination_connection_port: self.destination_runtime_postgres_port(),
                    runner_container_ip: Some(process.container_ip()),
                    postgres_apply_client_addr: apply_client_addr.clone(),
                },
                None => panic!("runner must be started before collecting runtime-shape audit"),
            }
        };
        RuntimeShapeAudit::new(
            destination_runtime,
            SourceBootstrapAudit::new(self.webhook_sink_base_url.clone()),
            CockroachRuntimeAudit::new(self.docker.cockroach_image()),
            DestinationRoleAudit::new(
                self.config.destination_user.clone(),
                self.destination_role_is_superuser(),
            ),
        )
    }

    pub fn post_setup_source_audit(&self) -> PostSetupSourceAudit {
        self.source_command_audit().post_setup()
    }

    pub fn assert_runner_alive(&self) {
        self.assert_runner_process_alive();
    }

    pub fn arm_single_external_sink_fault_for_request_body(
        &self,
        body_substring: &str,
        fault: ExternalSinkFault,
    ) {
        self.webhook_chaos_gateway
            .as_ref()
            .expect("chaos gateway should be configured for this harness")
            .arm_single_external_fault_for_body_substring(body_substring, fault);
    }

    pub fn wait_for_duplicate_gateway_delivery_of_request_body(&self, body_substring: &str) {
        for _ in 0..120 {
            self.assert_runner_process_alive();
            let gateway = self
                .webhook_chaos_gateway
                .as_ref()
                .expect("chaos gateway should be configured for this harness");
            if gateway.has_duplicate_delivery_for_body_substring(body_substring) {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        let gateway = self
            .webhook_chaos_gateway
            .as_ref()
            .expect("chaos gateway should be configured for this harness");
        panic!(
            "gateway did not observe duplicate delivery for request body containing `{body_substring}`\nattempts={}\nrunner stderr:\n{}",
            gateway.attempt_summary_for_body_substring(body_substring),
            self.runner_diagnostics(),
        );
    }

    pub fn wait_for_gateway_fault_then_success_for_request_body(
        &self,
        body_substring: &str,
        fault: ExternalSinkFault,
    ) {
        for _ in 0..120 {
            self.assert_runner_process_alive();
            let gateway = self
                .webhook_chaos_gateway
                .as_ref()
                .expect("chaos gateway should be configured for this harness");
            if gateway.has_fault_then_forward_success_for_body_substring(body_substring, fault) {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        let gateway = self
            .webhook_chaos_gateway
            .as_ref()
            .expect("chaos gateway should be configured for this harness");
        panic!(
            "gateway did not observe `{fault}` followed by a successful forward for request body containing `{body_substring}`\nattempts={}\nrunner stderr:\n{}",
            gateway.attempt_summary_for_body_substring(body_substring),
            self.runner_diagnostics(),
        );
    }

    pub fn wait_for_gateway_forwarded_status_sequence_for_request_body(
        &self,
        body_substring: &str,
        statuses: &[reqwest::StatusCode],
    ) {
        for _ in 0..120 {
            self.assert_runner_process_alive();
            let gateway = self
                .webhook_chaos_gateway
                .as_ref()
                .expect("chaos gateway should be configured for this harness");
            if gateway.has_forwarded_downstream_status_sequence_for_body_substring(
                body_substring,
                statuses,
            ) {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        let gateway = self
            .webhook_chaos_gateway
            .as_ref()
            .expect("chaos gateway should be configured for this harness");
        let expected = statuses
            .iter()
            .map(|status| status.as_str().to_owned())
            .collect::<Vec<_>>()
            .join(" -> ");
        panic!(
            "gateway did not observe forwarded downstream status sequence `{expected}` for request body containing `{body_substring}`\nattempts={}\nrunner stderr:\n{}",
            gateway.attempt_summary_for_body_substring(body_substring),
            self.runner_diagnostics(),
        );
    }

    pub(crate) fn apply_source_workload_batch(&self, sql: &str) {
        run_audited_cockroach_sql(
            &self.wrapper_bin_dir,
            &format!("USE {};\n{sql}", self.config.source_database),
        );
    }

    pub fn query_destination(&self, sql: &str) -> String {
        self.docker
            .exec_psql(&self.config.destination_database, sql)
    }

    pub fn helper_tables(&self) -> String {
        self.docker.exec_psql(
            &self.config.destination_database,
            "SELECT string_agg(table_name, ',' ORDER BY table_name)
             FROM information_schema.tables
             WHERE table_schema = '_cockroach_migration_tool';",
        )
    }

    pub fn helper_table_row_count(&self, mapped_table: &str) -> usize {
        let row_count = self.docker.exec_psql(
            &self.config.destination_database,
            &format!(
                "SELECT count(*)::text
                 FROM _cockroach_migration_tool.\"{}\";",
                self.helper_table_name(mapped_table)
            ),
        );
        row_count
            .trim()
            .parse::<usize>()
            .expect("helper shadow row count should parse")
    }

    pub fn destination_constraint_snapshot(&self) -> String {
        let selected_table_names = self
            .config
            .selected_tables
            .iter()
            .map(|table| {
                let (_, table_name) = split_table_reference(table);
                format!("'{}'", table_name.replace('\'', "''"))
            })
            .collect::<Vec<_>>()
            .join(", ");
        self.query_destination(&format!(
            "SELECT COALESCE(
                 string_agg(
                     table_name || ':' || constraint_name || ':' || constraint_type,
                     ',' ORDER BY table_name, constraint_name
                 ),
                 '<empty>'
             )
             FROM information_schema.table_constraints
             WHERE table_schema = 'public'
               AND table_name IN ({selected_table_names})
               AND constraint_type IN ('PRIMARY KEY', 'FOREIGN KEY');"
        ))
    }

    pub fn tracking_progress(&self, mapped_table: &str) -> MappingTrackingProgress {
        let mapping_id = sql_string_literal(&self.config.mapping_id);
        let mapped_table = sql_string_literal(mapped_table);
        let stream = self.query_destination(&format!(
            "SELECT json_build_object(
                 'latest_received_resolved_watermark', latest_received_resolved_watermark,
                 'latest_reconciled_resolved_watermark', latest_reconciled_resolved_watermark
             )::text
             FROM _cockroach_migration_tool.stream_state
             WHERE mapping_id = '{mapping_id}';"
        ));
        let table = self.query_destination(&format!(
            "SELECT json_build_object(
                 'source_table_name', source_table_name,
                 'helper_table_name', helper_table_name,
                 'last_successful_sync_watermark', last_successful_sync_watermark,
                 'last_error', last_error
             )::text
             FROM _cockroach_migration_tool.table_sync_state
             WHERE mapping_id = '{mapping_id}'
               AND source_table_name = '{mapped_table}';"
        ));
        let stream = parse_json_snapshot::<StreamTrackingProgress>(
            stream.trim(),
            "stream tracking snapshot",
        );
        let table =
            parse_json_snapshot::<TableTrackingProgress>(table.trim(), "table tracking snapshot");
        MappingTrackingProgress { stream, table }
    }

    pub fn lock_destination_table(&self, mapped_table: &str) -> DestinationTableLock {
        let suffix = mapped_table.replace('.', "__");
        DestinationTableLock::acquire(
            self.docker.postgres_host_port,
            &self.config.destination_database,
            mapped_table,
            &format!("{suffix}-{}", unique_suffix()),
            &self
                .temp_dir
                .path()
                .join(format!("{suffix}.lock.stdout.log")),
            &self
                .temp_dir
                .path()
                .join(format!("{suffix}.lock.stderr.log")),
        )
    }

    pub fn wait_for_tracking_progress<F>(
        &self,
        mapped_table: &str,
        description: &str,
        mut predicate: F,
    ) -> MappingTrackingProgress
    where
        F: FnMut(&MappingTrackingProgress) -> bool,
    {
        for _ in 0..120 {
            self.assert_runner_process_alive();
            let progress = self.tracking_progress(mapped_table);
            if predicate(&progress) {
                return progress;
            }
            thread::sleep(Duration::from_secs(1));
        }

        let actual = self.tracking_progress(mapped_table);
        panic!(
            "{description} did not converge\nactual progress={actual:?}\nrunner stderr:\n{}",
            self.runner_diagnostics(),
        );
    }

    pub fn install_destination_write_failure(
        &self,
        schema_name: &str,
        table_name: &str,
        row_predicate_sql: &str,
        error_message: &str,
    ) -> DestinationWriteFailure {
        DestinationWriteFailure::install(
            self.docker.postgres_host_port,
            &self.config.destination_database,
            DestinationWriteFailureSpec {
                schema_name,
                table_name,
                row_predicate_sql,
                error_message,
            },
        )
    }

    pub fn helper_table_name_for(&self, mapped_table: &str) -> String {
        self.helper_table_name(mapped_table)
    }

    pub fn verify_selected_tables_via_image(
        &self,
        verify_image: &VerifyImageHarness,
    ) -> VerifyCorrectnessAudit {
        let (include_schema_pattern, include_table_pattern) =
            verify_filter_patterns(&self.config.selected_tables);
        verify_image.run_correctness_audit(&VerifyImageRun {
            network_name: self.docker.network_name.clone(),
            source_url: self.docker.verify_source_url(&self.config.source_database),
            source_ca_cert_path: self.docker.cockroach_ca_cert_path(),
            source_client_cert_path: self.docker.cockroach_client_cert_path(),
            source_client_key_path: self.docker.cockroach_client_key_path(),
            destination_url: self.docker.verify_destination_url(
                &self.config.destination_database,
                &self.config.destination_user,
                &self.config.destination_password,
            ),
            destination_ca_cert_path: self.docker.postgres_ca_cert_path(),
            include_schema_pattern,
            include_table_pattern,
            expected_tables: self.config.selected_tables.clone(),
        })
    }

    pub fn wait_for_selected_tables_to_match_via_image(
        &self,
        verify_image: &VerifyImageHarness,
        description: &str,
    ) -> VerifyCorrectnessAudit {
        for _ in 0..60 {
            self.assert_runner_process_alive();
            let audit = self.verify_selected_tables_via_image(verify_image);
            if audit.selected_tables_match() {
                return audit;
            }
            thread::sleep(Duration::from_secs(1));
        }

        let audit = self.verify_selected_tables_via_image(verify_image);
        panic!(
            "{description} did not converge through the verify image correctness boundary\nfinal audit={audit:?}\nrunner stderr:\n{}",
            self.runner_diagnostics(),
        );
    }

    pub fn assert_selected_tables_match_via_image_stable(
        &self,
        verify_image: &VerifyImageHarness,
        description: &str,
        duration: Duration,
    ) {
        let deadline = Instant::now() + duration;
        loop {
            self.assert_runner_process_alive();
            let audit = self.verify_selected_tables_via_image(verify_image);
            assert!(
                audit.selected_tables_match(),
                "{description} stopped matching through the verify image correctness boundary\nfinal audit={audit:?}\nrunner stderr:\n{}",
                self.runner_diagnostics(),
            );
            if Instant::now() >= deadline {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    pub fn wait_for_runner_failed_exit(&self) -> String {
        let mut runner_process = self.runner_process.borrow_mut();
        let process = runner_process
            .take()
            .expect("runner process should exist before waiting for a failed exit");
        match process {
            RunnerRuntime::Host(mut process) => process.wait_for_failed_exit(),
            RunnerRuntime::Container(process) => process.wait_for_failed_exit(),
        }
    }

    pub fn wait_for_reconcile_block_on_destination_table(&self, mapped_table: &str) {
        let destination_database = sql_string_literal(&self.config.destination_database);
        let destination_user = sql_string_literal(&self.config.destination_user);
        let (schema_name, table_name) = split_table_reference(mapped_table);
        let quoted_upsert_prefix =
            sql_string_literal(&format!("INSERT INTO \"{schema_name}\".\"{table_name}\""));
        for _ in 0..120 {
            self.assert_runner_process_alive();
            let blocked_sessions = self.query_destination(&format!(
                "SELECT count(*)::text
                 FROM pg_stat_activity
                 WHERE datname = '{destination_database}'
                   AND usename = '{destination_user}'
                   AND wait_event_type = 'Lock'
                   AND state = 'active'
                   AND position('{quoted_upsert_prefix}' IN query) > 0;"
            ));
            if blocked_sessions.trim() != "0" {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "reconcile did not block on destination table `{mapped_table}`\npg_stat_activity:\n{}\nrunner stderr:\n{}",
            self.query_destination(
                "SELECT string_agg(
                     pid::text || ':' ||
                     usename || ':' ||
                     COALESCE(wait_event_type, '<null>') || ':' ||
                     COALESCE(state, '<null>') || ':' ||
                     regexp_replace(query, '\\s+', ' ', 'g'),
                     E'\\n' ORDER BY pid
                 )
                 FROM pg_stat_activity
                 WHERE datname = current_database();"
            )
            .trim(),
            self.runner_diagnostics(),
        );
    }

    fn destination_runtime_client_addr(&self) -> Option<String> {
        let destination_user = sql_string_literal(&self.config.destination_user);
        let client_addr = self.query_destination(&format!(
            "SELECT COALESCE(client_addr::text, '')
             FROM pg_stat_activity
             WHERE datname = current_database()
               AND usename = '{destination_user}'
               AND pid <> pg_backend_pid()
             ORDER BY backend_start
             LIMIT 1;"
        ));
        let client_addr = client_addr.trim();
        if client_addr.is_empty() {
            None
        } else {
            Some(client_addr.to_owned())
        }
    }

    fn destination_role_is_superuser(&self) -> bool {
        let destination_user = sql_string_literal(&self.config.destination_user);
        let is_superuser = self.query_destination(&format!(
            "SELECT rolsuper::text
             FROM pg_roles
             WHERE rolname = '{destination_user}';"
        ));
        match is_superuser.trim() {
            "t" | "true" => true,
            "f" | "false" => false,
            other => panic!("rolsuper query should return `t` or `f`, got `{other}`"),
        }
    }

    fn materialize(mut self, webhook_sink_mode: WebhookSinkMode) -> Self {
        self.runner_config_path = self.temp_dir.path().join("runner.yml");
        self.source_bootstrap_config_path = self.temp_dir.path().join("cockroach-setup.yml");
        self.cockroach_wrapper_log_path = self.temp_dir.path().join("cockroach-wrapper.log");
        self.runner_stdout_path = self.temp_dir.path().join("runner.stdout.log");
        self.runner_stderr_path = self.temp_dir.path().join("runner.stderr.log");

        write_cockroach_wrapper_script(
            &self.wrapper_bin_dir.join("cockroach"),
            &self.cockroach_wrapper_log_path,
            &self.docker.cockroach_container,
        );
        match webhook_sink_mode {
            WebhookSinkMode::DirectRunner => {
                self.webhook_sink_base_url =
                    format!("https://host.docker.internal:{}", self.runner_port);
            }
            WebhookSinkMode::ObservableChaosGateway => {
                let gateway = WebhookChaosGateway::start(self.runner_port);
                self.webhook_sink_base_url = gateway.public_base_url();
                self.webhook_chaos_gateway = Some(gateway);
            }
        }
        self.write_runner_config();
        self.write_source_bootstrap_config();
        self
    }

    fn start_runner_process(&self) {
        if self.runner_process.borrow().is_some() {
            return;
        }

        let child = match self.destination_runtime_mode {
            DestinationRuntimeMode::HostProcess => RunnerRuntime::Host(RunnerProcess::start(
                &self.runner_config_path,
                &self.runner_stdout_path,
                &self.runner_stderr_path,
            )),
            DestinationRuntimeMode::SingleContainer => {
                RunnerRuntime::Container(RunnerContainerProcess::start(
                    &self.docker.network_name,
                    self.runner_port,
                    &self.runner_config_path,
                ))
            }
        };
        *self.runner_process.borrow_mut() = Some(child);
    }

    fn render_source_bootstrap_sql(&self) -> String {
        source_bootstrap::execute(source_bootstrap::Cli::parse_from([
            "setup-sql",
            "emit-cockroach-sql",
            "--config",
            self.source_bootstrap_config_path
                .to_str()
                .expect("Cockroach setup config path should be utf-8"),
        ]))
        .unwrap_or_else(|error| panic!("setup-sql emit-cockroach-sql failed: {error}"))
    }

    fn apply_source_bootstrap_sql(&self, sql: &str) {
        for statement in source_bootstrap_sql_statements(sql) {
            run_audited_cockroach_sql(&self.wrapper_bin_dir, &statement);
        }
    }

    fn write_runner_config(&self) {
        let selected_tables = self
            .config
            .selected_tables
            .iter()
            .map(|table| format!("        - {table}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(
            &self.runner_config_path,
            format!(
                r#"webhook:
  bind_addr: 0.0.0.0:{runner_bind_port}
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
reconcile:
  interval_secs: {reconcile_interval_secs}
mappings:
  - id: {mapping_id}
    source:
      database: {source_database}
      tables:
{selected_tables}
    destination:
      connection:
        host: {destination_host}
        port: {postgres_port}
        database: {destination_database}
        user: {destination_user}
        password: {destination_password}
"#,
                runner_bind_port = self.destination_runtime_bind_port(),
                cert_path = investigation_server_cert_path().display(),
                key_path = investigation_server_key_path().display(),
                mapping_id = self.config.mapping_id,
                source_database = self.config.source_database,
                selected_tables = selected_tables,
                destination_host = self.destination_runtime_postgres_host(),
                postgres_port = self.destination_runtime_postgres_port(),
                destination_database = self.config.destination_database,
                destination_user = self.config.destination_user,
                destination_password = self.config.destination_password,
                reconcile_interval_secs = self.config.reconcile_interval_secs,
            ),
        )
        .expect("runner config should be written");
    }

    fn write_source_bootstrap_config(&self) {
        let selected_tables = self
            .config
            .selected_tables
            .iter()
            .map(|table| format!("        - {table}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(
            &self.source_bootstrap_config_path,
            format!(
                r#"cockroach:
  url: {cockroach_url}
webhook:
  base_url: {webhook_sink_base_url}
  ca_cert_path: {ca_cert_path}
  resolved: 1s
mappings:
  - id: {mapping_id}
    source:
      database: {source_database}
      tables:
{selected_tables}
"#,
                cockroach_url = self
                    .docker
                    .source_bootstrap_cockroach_url(&self.config.source_database),
                webhook_sink_base_url = self.webhook_sink_base_url,
                ca_cert_path = investigation_ca_cert_path().display(),
                mapping_id = self.config.mapping_id,
                source_database = self.config.source_database,
                selected_tables = selected_tables,
            ),
        )
        .expect("Cockroach setup config should be written");
    }

    fn assert_runner_process_alive(&self) {
        let mut process = self.runner_process.borrow_mut();
        let runner_process = process
            .as_mut()
            .expect("runner runtime should exist before asserting liveness");

        match runner_process {
            RunnerRuntime::Host(process) => process.assert_alive(),
            RunnerRuntime::Container(process) => process.assert_alive(),
        }
    }

    fn runner_logs(&self) -> String {
        match self.runner_process.borrow().as_ref() {
            Some(RunnerRuntime::Host(_)) => format!(
                "stdout:\n{}\n\nstderr:\n{}",
                read_file(&self.runner_stdout_path),
                read_file(&self.runner_stderr_path),
            ),
            Some(RunnerRuntime::Container(process)) => process.logs(),
            None => String::new(),
        }
    }

    fn runner_diagnostics(&self) -> String {
        self.runner_logs()
    }

    fn destination_runtime_postgres_host(&self) -> &'static str {
        match self.destination_runtime_mode {
            DestinationRuntimeMode::HostProcess => "127.0.0.1",
            DestinationRuntimeMode::SingleContainer => "postgres",
        }
    }

    fn destination_runtime_postgres_port(&self) -> u16 {
        match self.destination_runtime_mode {
            DestinationRuntimeMode::HostProcess => self.docker.postgres_host_port,
            DestinationRuntimeMode::SingleContainer => 5432,
        }
    }

    fn destination_runtime_bind_port(&self) -> u16 {
        match self.destination_runtime_mode {
            DestinationRuntimeMode::HostProcess => self.runner_port,
            DestinationRuntimeMode::SingleContainer => 8443,
        }
    }

    fn helper_table_name(&self, mapped_table: &str) -> String {
        let (schema, table) = split_table_reference(mapped_table);
        format!("{}__{}__{}", self.config.mapping_id, schema, table)
    }

    fn source_command_audit(&self) -> SourceCommandAudit {
        let bootstrap_command_count = self
            .bootstrap_source_command_count
            .borrow()
            .as_ref()
            .copied()
            .expect("bootstrap migration must finish before collecting source-command audit");
        SourceCommandAudit::from_cockroach_log(
            &self.cockroach_wrapper_log_path,
            bootstrap_command_count,
        )
    }

    fn read_source_command_count(&self) -> usize {
        SourceCommandAudit::from_cockroach_log(&self.cockroach_wrapper_log_path, 0).command_count()
    }
}

pub(crate) struct DockerEnvironment {
    _tls_material_dir: TempDir,
    network_name: String,
    pub(crate) cockroach_container: String,
    postgres_container: String,
    cockroach_host_port: u16,
    pub(crate) postgres_host_port: u16,
    cockroach_certs_dir: PathBuf,
    postgres_tls_dir: PathBuf,
}

impl DockerEnvironment {
    pub(crate) fn new() -> Self {
        let suffix = unique_suffix();
        let tls_material_dir = tempfile::tempdir().expect("tls material dir should be created");
        let cockroach_certs_dir = tls_material_dir.path().join("cockroach-certs");
        let postgres_tls_dir = tls_material_dir.path().join("postgres-tls");
        fs::create_dir_all(&cockroach_certs_dir).expect("cockroach cert dir should be created");
        fs::create_dir_all(&postgres_tls_dir).expect("postgres tls dir should be created");
        generate_cockroach_certs(&cockroach_certs_dir);
        generate_postgres_tls_material(&postgres_tls_dir);
        Self {
            _tls_material_dir: tls_material_dir,
            network_name: "cockroach-migrate-runner-e2e-shared".to_owned(),
            cockroach_container: format!("cockroach-migrate-cockroach-{suffix}"),
            postgres_container: format!("cockroach-migrate-postgres-{suffix}"),
            cockroach_host_port: pick_unused_port(),
            postgres_host_port: pick_unused_port(),
            cockroach_certs_dir,
            postgres_tls_dir,
        }
    }

    pub(crate) fn create_network(&self) {
        if Command::new("docker")
            .args(["network", "inspect", &self.network_name])
            .output()
            .expect("docker network inspect should start")
            .status
            .success()
        {
            return;
        }
        run_command_capture(
            Command::new("docker").args(["network", "create", &self.network_name]),
            "docker network create",
        );
    }

    pub(crate) fn start_cockroach(&self) {
        let cert_mount = format!("{}:/certs:ro", self.cockroach_certs_dir.display());
        run_command_capture(
            Command::new("docker").args([
                "run",
                "-d",
                "--name",
                &self.cockroach_container,
                "--network",
                &self.network_name,
                "--network-alias",
                "cockroach",
                "--add-host",
                "host.docker.internal:host-gateway",
                "-p",
                &format!("127.0.0.1:{}:26258", self.cockroach_host_port),
                "-v",
                &cert_mount,
                COCKROACH_IMAGE,
                "start-single-node",
                "--certs-dir=/certs",
                "--listen-addr=localhost:26257",
                "--advertise-addr=localhost:26257",
                "--sql-addr=0.0.0.0:26258",
                "--advertise-sql-addr=localhost:26258",
                "--http-addr=0.0.0.0:8080",
            ]),
            "docker run cockroach",
        );
    }

    pub(crate) fn start_postgres(&self) {
        run_command_capture(
            Command::new("docker").args([
                "run",
                "-d",
                "--rm",
                "--name",
                &self.postgres_container,
                "--network",
                &self.network_name,
                "--network-alias",
                "postgres",
                "-p",
                &format!("127.0.0.1:{}:5432", self.postgres_host_port),
                "-e",
                "POSTGRES_USER=postgres",
                "-e",
                "POSTGRES_PASSWORD=postgres",
                "-e",
                "POSTGRES_DB=postgres",
                POSTGRES_IMAGE,
            ]),
            "docker run postgres",
        );
    }

    pub(crate) fn wait_for_cockroach(&self) {
        for _ in 0..60 {
            let status = Command::new("docker")
                .args([
                    "exec",
                    &self.cockroach_container,
                    "cockroach",
                    "sql",
                    "--certs-dir=/certs",
                    "--host=localhost:26258",
                    "-e",
                    "select 1",
                ])
                .status()
                .expect("docker exec cockroach should start");
            if status.success() {
                return;
            }
            if !container_running(&self.cockroach_container) {
                panic!(
                    "cockroach container exited during startup\n{}",
                    docker_logs(&self.cockroach_container)
                );
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "cockroach container did not become ready\n{}",
            docker_logs(&self.cockroach_container)
        );
    }

    pub(crate) fn wait_for_postgres(&self) {
        for _ in 0..60 {
            let status = Command::new("docker")
                .args([
                    "exec",
                    "-e",
                    "PGPASSWORD=postgres",
                    &self.postgres_container,
                    "pg_isready",
                    "-h",
                    "127.0.0.1",
                    "-U",
                    "postgres",
                    "-d",
                    "postgres",
                ])
                .status()
                .expect("docker exec pg_isready should start");
            if status.success() {
                self.enable_postgres_ssl();
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!("postgres container did not become ready");
    }

    pub(crate) fn prepare_source_schema_and_seed(&self, sql: &str) {
        self.exec_cockroach_sql(sql);
    }

    pub(crate) fn prepare_destination_database(
        &self,
        destination_database: &str,
        destination_user: &str,
        destination_password: &str,
        destination_setup_sql: &str,
    ) {
        self.exec_psql(
            "postgres",
            &format!(
                "CREATE ROLE {destination_user} LOGIN PASSWORD '{password}';",
                password = destination_password.replace('\'', "''"),
            ),
        );
        self.exec_psql(
            "postgres",
            &format!("CREATE DATABASE {destination_database} OWNER {destination_user};"),
        );
        self.exec_psql(
            destination_database,
            &format!(
                "SET ROLE {destination_user};
                 {destination_setup_sql}"
            ),
        );
    }

    pub(crate) fn exec_cockroach_sql(&self, sql: &str) -> String {
        run_command_capture(
            Command::new("docker").args([
                "exec",
                &self.cockroach_container,
                "cockroach",
                "sql",
                "--certs-dir=/certs",
                "--host=localhost:26258",
                "--format=csv",
                "-e",
                sql,
            ]),
            "docker exec cockroach sql",
        )
    }

    pub(crate) fn exec_psql(&self, database: &str, sql: &str) -> String {
        run_command_capture(
            Command::new("docker").args([
                "exec",
                "-e",
                "PGPASSWORD=postgres",
                &self.postgres_container,
                "psql",
                "-h",
                "127.0.0.1",
                "-U",
                "postgres",
                "-d",
                database,
                "-v",
                "ON_ERROR_STOP=1",
                "-t",
                "-A",
                "-c",
                sql,
            ]),
            "docker exec psql",
        )
    }

    pub(crate) fn cockroach_image(&self) -> String {
        docker_inspect_format(&self.cockroach_container, "{{.Config.Image}}")
    }

    fn enable_postgres_ssl(&self) {
        copy_file_into_container(
            self.postgres_tls_dir.join("server.crt").as_path(),
            &self.postgres_container,
            "/var/lib/postgresql/data/server.crt",
            "docker cp postgres server cert",
        );
        copy_file_into_container(
            self.postgres_tls_dir.join("server.key").as_path(),
            &self.postgres_container,
            "/var/lib/postgresql/data/server.key",
            "docker cp postgres server key",
        );
        run_command_capture(
            Command::new("docker").args([
                "exec",
                "-u",
                "0",
                &self.postgres_container,
                "bash",
                "-lc",
                "set -euo pipefail\n\
                 chown postgres:postgres /var/lib/postgresql/data/server.crt /var/lib/postgresql/data/server.key\n\
                 chmod 600 /var/lib/postgresql/data/server.key\n\
                 printf '\\nssl=on\\nssl_cert_file='\"'\"'/var/lib/postgresql/data/server.crt'\"'\"'\\nssl_key_file='\"'\"'/var/lib/postgresql/data/server.key'\"'\"'\\n' >> /var/lib/postgresql/data/postgresql.conf",
            ]),
            "docker exec postgres enable ssl",
        );
        run_command_capture(
            Command::new("docker").args(["restart", &self.postgres_container]),
            "docker restart postgres after ssl enable",
        );
        for _ in 0..60 {
            let status = Command::new("docker")
                .args([
                    "exec",
                    "-e",
                    "PGPASSWORD=postgres",
                    &self.postgres_container,
                    "pg_isready",
                    "-h",
                    "127.0.0.1",
                    "-U",
                    "postgres",
                    "-d",
                    "postgres",
                ])
                .status()
                .expect("docker exec pg_isready should start");
            if status.success() {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "postgres container did not become ready after enabling ssl\n{}",
            docker_logs(&self.postgres_container)
        );
    }

    pub(crate) fn source_bootstrap_cockroach_url(&self, database: &str) -> String {
        format!(
            "postgresql://root@127.0.0.1:{port}/{database}?sslmode=verify-full&sslrootcert={ca}&sslcert={client_cert}&sslkey={client_key}",
            port = self.cockroach_host_port,
            database = database,
            ca = self.cockroach_ca_cert_path().display(),
            client_cert = self.cockroach_client_cert_path().display(),
            client_key = self.cockroach_client_key_path().display(),
        )
    }

    pub(crate) fn verify_source_url(&self, database: &str) -> String {
        format!("postgresql://root@cockroach:26258/{database}")
    }

    pub(crate) fn network_name(&self) -> &str {
        &self.network_name
    }

    pub(crate) fn verify_destination_url(
        &self,
        database: &str,
        user: &str,
        password: &str,
    ) -> String {
        format!(
            "postgresql://{user}:{password}@postgres:5432/{database}",
            user = user,
            password = password,
            database = database,
        )
    }

    pub(crate) fn cockroach_ca_cert_path(&self) -> PathBuf {
        self.cockroach_certs_dir.join("ca.crt")
    }

    pub(crate) fn cockroach_client_cert_path(&self) -> PathBuf {
        self.cockroach_certs_dir.join("client.root.crt")
    }

    pub(crate) fn cockroach_client_key_path(&self) -> PathBuf {
        self.cockroach_certs_dir.join("client.root.key")
    }

    pub(crate) fn postgres_ca_cert_path(&self) -> PathBuf {
        self.postgres_tls_dir.join("ca.crt")
    }
}

impl Drop for DockerEnvironment {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.postgres_container])
            .output();
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.cockroach_container])
            .output();
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

pub(crate) fn investigation_ca_cert_path() -> PathBuf {
    repo_root()
        .join("investigations")
        .join("cockroach-webhook-cdc")
        .join("certs")
        .join("ca.crt")
}

pub(crate) fn investigation_server_cert_path() -> PathBuf {
    repo_root()
        .join("investigations")
        .join("cockroach-webhook-cdc")
        .join("certs")
        .join("server.crt")
}

pub(crate) fn investigation_server_key_path() -> PathBuf {
    repo_root()
        .join("investigations")
        .join("cockroach-webhook-cdc")
        .join("certs")
        .join("server.key")
}

fn generate_cockroach_certs(certs_dir: &Path) {
    let mount = format!("{}:/certs", certs_dir.display());
    let user = current_user_spec();
    run_command_capture(
        Command::new("docker").args([
            "run",
            "--rm",
            "--user",
            &user,
            "-v",
            &mount,
            COCKROACH_IMAGE,
            "cert",
            "create-ca",
            "--certs-dir=/certs",
            "--ca-key=/certs/ca.key",
        ]),
        "docker run cockroach cert create-ca",
    );
    run_command_capture(
        Command::new("docker").args([
            "run",
            "--rm",
            "--user",
            &user,
            "-v",
            &mount,
            COCKROACH_IMAGE,
            "cert",
            "create-node",
            "localhost",
            "127.0.0.1",
            "cockroach",
            "--certs-dir=/certs",
            "--ca-key=/certs/ca.key",
        ]),
        "docker run cockroach cert create-node",
    );
    run_command_capture(
        Command::new("docker").args([
            "run",
            "--rm",
            "--user",
            &user,
            "-v",
            &mount,
            COCKROACH_IMAGE,
            "cert",
            "create-client",
            "root",
            "--certs-dir=/certs",
            "--ca-key=/certs/ca.key",
        ]),
        "docker run cockroach cert create-client",
    );
}

fn generate_postgres_tls_material(tls_dir: &Path) {
    let server_config_path = tls_dir.join("server.cnf");
    fs::write(
        &server_config_path,
        "[req]\n\
         distinguished_name = dn\n\
         prompt = no\n\
         req_extensions = req_ext\n\
         \n\
         [dn]\n\
         CN = postgres\n\
         \n\
         [req_ext]\n\
         subjectAltName = @alt_names\n\
         \n\
         [alt_names]\n\
         DNS.1 = postgres\n\
         DNS.2 = localhost\n\
         IP.1 = 127.0.0.1\n",
    )
    .expect("postgres tls config should be written");
    run_command_capture(
        Command::new("openssl").args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-days",
            "365",
            "-nodes",
            "-keyout",
            tls_dir
                .join("ca.key")
                .to_str()
                .expect("postgres ca key path should be utf-8"),
            "-out",
            tls_dir
                .join("ca.crt")
                .to_str()
                .expect("postgres ca cert path should be utf-8"),
            "-subj",
            "/CN=runner-postgres-ca",
        ]),
        "openssl req postgres ca",
    );
    run_command_capture(
        Command::new("openssl").args([
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            tls_dir
                .join("server.key")
                .to_str()
                .expect("postgres server key path should be utf-8"),
            "-out",
            tls_dir
                .join("server.csr")
                .to_str()
                .expect("postgres server csr path should be utf-8"),
            "-config",
            server_config_path
                .to_str()
                .expect("postgres server config path should be utf-8"),
        ]),
        "openssl req postgres server cert request",
    );
    run_command_capture(
        Command::new("openssl").args([
            "x509",
            "-req",
            "-days",
            "365",
            "-in",
            tls_dir
                .join("server.csr")
                .to_str()
                .expect("postgres server csr path should be utf-8"),
            "-CA",
            tls_dir
                .join("ca.crt")
                .to_str()
                .expect("postgres ca cert path should be utf-8"),
            "-CAkey",
            tls_dir
                .join("ca.key")
                .to_str()
                .expect("postgres ca key path should be utf-8"),
            "-CAcreateserial",
            "-out",
            tls_dir
                .join("server.crt")
                .to_str()
                .expect("postgres server cert path should be utf-8"),
            "-extensions",
            "req_ext",
            "-extfile",
            server_config_path
                .to_str()
                .expect("postgres server config path should be utf-8"),
        ]),
        "openssl x509 postgres server cert",
    );
}

fn copy_file_into_container(source: &Path, container: &str, destination: &str, context: &str) {
    run_command_capture(
        Command::new("docker").args([
            "cp",
            source.to_str().expect("copy source path should be utf-8"),
            &format!("{container}:{destination}"),
        ]),
        context,
    );
}

fn current_user_spec() -> String {
    let uid = run_command_capture(Command::new("id").arg("-u"), "id -u")
        .trim()
        .to_owned();
    let gid = run_command_capture(Command::new("id").arg("-g"), "id -g")
        .trim()
        .to_owned();
    format!("{uid}:{gid}")
}

fn unique_suffix() -> String {
    static UNIQUE_SUFFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos(),
        UNIQUE_SUFFIX_COUNTER.fetch_add(1, Ordering::Relaxed),
    )
}

fn e2e_docker_resource_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(crate) fn lock_e2e_docker_resources() -> MutexGuard<'static, ()> {
    e2e_docker_resource_lock()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

pub(crate) fn pick_unused_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("ephemeral port should bind")
        .local_addr()
        .expect("bound socket should have a local address")
        .port()
}

pub(crate) fn write_cockroach_wrapper_script(path: &Path, log_path: &Path, container_name: &str) {
    fs::write(
        path,
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nfor arg in \"$@\"; do\n  escaped_arg=${{arg//\\\\/\\\\\\\\}}\n  escaped_arg=${{escaped_arg//$'\\n'/\\\\n}}\n  escaped_arg=${{escaped_arg//$'\\t'/\\\\t}}\n  printf 'ARG_ESC\\t%s\\n' \"$escaped_arg\" >> {log_path}\ndone\nprintf 'END\\n' >> {log_path}\nexec docker exec {container_name} cockroach \"$@\"\n",
            log_path = shell_quote(log_path),
            container_name = shell_quote_text(container_name),
        ),
    )
    .expect("wrapper script should be written");
    make_executable(path);
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)
            .expect("file metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("file should be executable");
    }
}

pub(crate) fn https_client(certificate_path: &Path) -> Client {
    let certificate =
        Certificate::from_pem(&fs::read(certificate_path).expect("certificate should be readable"))
            .expect("certificate should parse");

    Client::builder()
        .add_root_certificate(certificate)
        .build()
        .expect("https client should build")
}

pub(crate) fn wait_for_runner_health<F>(client: &Client, port: u16, logs: F)
where
    F: Fn() -> String,
{
    for _ in 0..60 {
        match client
            .get(format!("https://localhost:{port}/healthz"))
            .send()
        {
            Ok(response) if response.status().is_success() => return,
            Ok(_) | Err(_) => thread::sleep(Duration::from_secs(1)),
        }
    }

    panic!(
        "runner did not become healthy on https://localhost:{port}/healthz\n{}",
        logs()
    );
}

pub(crate) fn run_audited_cockroach_sql(wrapper_bin_dir: &Path, sql: &str) -> String {
    run_command_capture(
        Command::new(wrapper_bin_dir.join("cockroach")).args([
            "sql",
            "--certs-dir=/certs",
            "--host=localhost:26258",
            "--format=csv",
            "-e",
            sql,
        ]),
        "audited cockroach sql",
    )
}

pub(crate) fn source_bootstrap_sql_statements(sql: &str) -> Vec<String> {
    sql.split(';')
        .map(str::trim)
        .filter(|statement| !statement.is_empty())
        .map(|statement| format!("{statement};"))
        .collect()
}

pub(crate) fn run_command_capture(command: &mut Command, context: &str) -> String {
    let output = run_command_output(command, context);
    String::from_utf8(output.stdout).expect("command stdout should be utf-8")
}

fn run_command_output(command: &mut Command, context: &str) -> std::process::Output {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{context} should start: {error}"));
    assert!(
        output.status.success(),
        "{context} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn docker_logs(container: &str) -> String {
    let output = Command::new("docker")
        .args(["logs", container])
        .output()
        .unwrap_or_else(|error| panic!("docker logs should start: {error}"));
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn docker_inspect_format(container: &str, format: &str) -> String {
    let output = Command::new("docker")
        .args(["inspect", "-f", format, container])
        .output()
        .unwrap_or_else(|error| panic!("docker inspect should start: {error}"));
    assert!(
        output.status.success(),
        "docker inspect failed for `{container}`:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout)
        .expect("docker inspect stdout should be utf-8")
        .trim()
        .to_owned()
}

fn container_running(container: &str) -> bool {
    let output = Command::new("docker")
        .args([
            "container",
            "inspect",
            "-f",
            "{{.State.Running}}",
            container,
        ])
        .output()
        .unwrap_or_else(|error| panic!("docker inspect should start: {error}"));
    output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
}

pub(crate) fn read_file(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => panic!("failed to read `{}`: {error}", path.display()),
    }
}

fn split_table_reference(table: &str) -> (&str, &str) {
    table
        .split_once('.')
        .unwrap_or_else(|| panic!("mapped table should be qualified as schema.table: {table}"))
}

fn verify_filter_patterns(selected_tables: &[String]) -> (String, String) {
    let mut schemas = selected_tables
        .iter()
        .map(|table| split_table_reference(table).0.to_owned())
        .collect::<Vec<_>>();
    schemas.sort();
    schemas.dedup();

    let mut tables = selected_tables
        .iter()
        .map(|table| split_table_reference(table).1.to_owned())
        .collect::<Vec<_>>();
    tables.sort();
    tables.dedup();

    (
        anchored_posix_union(&schemas),
        anchored_posix_union(&tables),
    )
}

fn anchored_posix_union(values: &[String]) -> String {
    format!("^({})$", values.join("|"))
}

fn parse_json_snapshot<T>(raw: &str, description: &str) -> T
where
    T: for<'de> Deserialize<'de>,
{
    assert!(
        !raw.is_empty(),
        "{description} query returned no rows when one was expected",
    );
    serde_json::from_str(raw)
        .unwrap_or_else(|error| panic!("{description} should parse from JSON `{raw}`: {error}"))
}

fn sql_string_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn watermark_at_least(actual: Option<&str>, expected: &str) -> bool {
    actual.is_some_and(|actual| actual >= expected)
}

fn watermark_has_advanced(actual: Option<&str>, earlier: Option<&str>) -> bool {
    match (actual, earlier) {
        (Some(actual), Some(earlier)) => actual > earlier,
        (Some(_), None) => true,
        _ => false,
    }
}

fn shell_quote(path: &Path) -> String {
    shell_quote_text(&path.display().to_string())
}

fn shell_quote_text(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}
