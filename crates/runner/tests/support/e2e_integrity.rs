use std::{collections::BTreeSet, fs, path::Path};

use serde::Deserialize;

const EXPECTED_COCKROACH_IMAGE: &str = "cockroachdb/cockroach:v26.1.2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomerLiveUpdateAudit {
    received_watermark: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyCorrectnessAudit {
    expected_tables: Vec<String>,
    job_id: String,
    status: String,
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
            table_summaries: result.table_summaries,
            mismatched_tables,
        }
    }

    pub fn assert_finished_successfully(&self) {
        assert!(
            self.finished_successfully(),
            "verify image job `{}` should succeed without hidden fallback verification: {:?}",
            self.job_id,
            self
        );
    }

    pub fn assert_selected_tables_match(&self) {
        assert!(
            self.selected_tables_match(),
            "verify image job `{}` should report selected-table correctness through the dedicated verify boundary: {:?}",
            self.job_id,
            self
        );
    }

    pub fn assert_detects_selected_table_mismatch(&self) {
        assert!(
            self.selected_tables_mismatch(),
            "verify image job `{}` should expose selected-table mismatches through the dedicated verify boundary: {:?}",
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
        self.finished_successfully()
            && self.covers_expected_tables()
            && (self.table_summaries.iter().any(|summary| {
                self.expected_tables.contains(&summary.table_name()) && summary.num_mismatch > 0
            }) || self
                .expected_tables
                .iter()
                .any(|expected_table| self.mismatched_tables.contains(expected_table)))
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
    mismatch_tables: Vec<VerifyTableRef>,
    #[serde(default)]
    table_definition_mismatches: Vec<VerifyTableDefinitionMismatch>,
}

impl VerifyJobResult {
    fn mismatched_tables(&self) -> BTreeSet<String> {
        self.mismatch_tables
            .iter()
            .map(VerifyTableRef::table_name)
            .chain(
                self.table_definition_mismatches
                    .iter()
                    .map(VerifyTableDefinitionMismatch::table_name),
            )
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

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct VerifyTableDefinitionMismatch {
    schema: String,
    table: String,
    message: String,
}

impl VerifyTableDefinitionMismatch {
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
        let commands = parse_logged_commands(log_path, "cockroach wrapper")
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
            "cockroach wrapper log should contain at least {bootstrap_command_count} audited source commands: {commands:?}",
        );

        Self {
            commands,
            bootstrap_command_count,
        }
    }

    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    pub fn assert_bootstrap_command_count(&self, expected: usize) {
        assert_eq!(
            self.bootstrap_commands().len(),
            expected,
            "unexpected audited bootstrap source command count: {:?}",
            self.bootstrap_commands(),
        );
    }

    pub fn assert_bootstrap_contains(&self, fragment: &str, description: &str) {
        assert!(
            self.bootstrap_commands()
                .iter()
                .any(|command| command.sql.contains(fragment)),
            "{description}: {:?}",
            self.bootstrap_commands(),
        );
    }

    pub fn assert_explicit_bootstrap_commands(
        &self,
        source_database: &str,
        expected_tables: &[&str],
    ) {
        self.assert_bootstrap_command_count(3);
        self.assert_bootstrap_contains(
            "SET CLUSTER SETTING kv.rangefeed.enabled = true;",
            "bootstrap should enable rangefeeds explicitly",
        );
        self.assert_bootstrap_contains(
            "SELECT cluster_logical_timestamp() AS changefeed_cursor;",
            "bootstrap should capture the start cursor explicitly",
        );
        let expected_changefeed_fragment = format!(
            "CREATE CHANGEFEED FOR TABLE {}",
            expected_tables
                .iter()
                .map(|table| format!("{source_database}.{table}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.assert_bootstrap_contains(
            &expected_changefeed_fragment,
            "bootstrap should create the expected changefeed explicitly",
        );
        self.assert_bootstrap_contains(
            "cursor = '",
            "bootstrap should pass the explicit cursor into the changefeed creation statement",
        );
    }

    pub fn post_setup(&self) -> PostSetupSourceAudit {
        let commands = self.commands[self.bootstrap_command_count..].to_vec();
        PostSetupSourceAudit { commands }
    }

    fn bootstrap_commands(&self) -> &[RecordedSourceCommand] {
        &self.commands[..self.bootstrap_command_count]
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
    source_bootstrap: SourceBootstrapAudit,
    cockroach: CockroachRuntimeAudit,
    destination_role: DestinationRoleAudit,
}

impl RuntimeShapeAudit {
    pub fn new(
        destination_runtime: DestinationRuntimeAudit,
        source_bootstrap: SourceBootstrapAudit,
        cockroach: CockroachRuntimeAudit,
        destination_role: DestinationRoleAudit,
    ) -> Self {
        Self {
            destination_runtime,
            source_bootstrap,
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
            self.destination_runtime.runner_entrypoint_json, None,
            "the honest default E2E path should not rely on a separate container entrypoint contract",
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
            self.source_bootstrap
                .webhook_sink_base_url
                .starts_with("https://"),
            "source bootstrap must target the destination runtime over HTTPS: {:?}",
            self.source_bootstrap,
        );
        assert_eq!(
            self.cockroach.image, EXPECTED_COCKROACH_IMAGE,
            "the honest default E2E path must use the real CockroachDB container image",
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
                Some(postgres_apply_client_addr),
                self.destination_runtime.runner_container_ip.as_deref(),
                "when PostgreSQL exposes a live runtime session, it must originate from the runner container",
            );
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestinationRuntimeAudit {
    pub(crate) mode: DestinationRuntimeMode,
    pub(crate) container_count: usize,
    pub(crate) runner_entrypoint_json: Option<String>,
    pub(crate) healthcheck_url: String,
    pub(crate) destination_connection_host: String,
    pub(crate) destination_connection_port: u16,
    pub(crate) runner_container_ip: Option<String>,
    pub(crate) postgres_apply_client_addr: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DestinationRuntimeMode {
    HostProcess,
    SingleContainer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceBootstrapAudit {
    webhook_sink_base_url: String,
}

impl SourceBootstrapAudit {
    pub fn new(webhook_sink_base_url: String) -> Self {
        Self {
            webhook_sink_base_url,
        }
    }
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
        "cockroach wrapper should be invoked with `sql`: {args:?}",
    );
    optional_flag_value(args, "-e")
        .or_else(|| optional_flag_value(args, "--execute"))
        .unwrap_or_else(|| panic!("cockroach wrapper should include `-e` or `--execute`: {args:?}"))
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
