use std::{collections::BTreeSet, fs, path::Path};

const EXPECTED_RUNNER_ENTRYPOINT_JSON: &str = "[\"/usr/local/bin/runner\"]";
const EXPECTED_COCKROACH_IMAGE: &str = "cockroachdb/cockroach:v26.1.2";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyAudit {
    output: String,
    command: VerifyCommandAudit,
}

impl VerifyAudit {
    pub fn from_runner_verify(
        output: String,
        log_path: &Path,
        expected_database: &str,
        expected_tables: &[&str],
    ) -> Self {
        let audit = Self {
            command: VerifyCommandAudit::from_log(log_path),
            output,
        };
        audit.assert_matches_runner_summary(expected_tables);
        audit.assert_targets_real_tables(expected_database, expected_tables);
        audit
    }

    pub fn assert_targets_real_tables(&self, expected_database: &str, expected_tables: &[&str]) {
        assert_eq!(
            self.command.target_database(),
            expected_database,
            "verify command should target the real destination database",
        );
        assert_eq!(
            self.command.schema_filter,
            expected_schema_filter(expected_tables),
            "verify command should filter the real mapped schemas only",
        );
        assert_eq!(
            self.command.table_filter,
            expected_table_filter(expected_tables),
            "verify command should filter the real mapped tables only",
        );
        assert!(
            !self.command.schema_filter.contains("_cockroach_migration_tool"),
            "verify command should never target helper schemas: {:?}",
            self.command,
        );
        assert!(
            !self.command.table_filter.contains("_cockroach_migration_tool"),
            "verify command should never target helper tables: {:?}",
            self.command,
        );
        assert!(
            self.command.allow_tls_mode_disable,
            "verify command should preserve the explicit TLS-mode override used by the E2E harness",
        );
    }

    pub fn assert_excludes_tables(&self, excluded_tables: &[&str]) {
        for table in excluded_tables {
            let (_, table_name) = split_table_reference(table);
            assert!(
                !self.output.contains(table),
                "verify output should not mention excluded table `{table}`: {}",
                self.output,
            );
            assert!(
                !self.command.table_filter.split('|').any(|entry| entry == table_name),
                "verify command should not target excluded table `{table}`: {:?}",
                self.command,
            );
        }
    }

    fn assert_matches_runner_summary(&self, expected_tables: &[&str]) {
        assert!(
            self.output.contains("verification"),
            "verify output should include a verification summary: {}",
            self.output,
        );
        assert!(
            self.output.contains("verdict=matched"),
            "verify output should report a matched verdict: {}",
            self.output,
        );
        assert!(
            self.output
                .contains(&format!("tables={}", expected_tables.join(","))),
            "verify output should mention only the real migrated tables: {}",
            self.output,
        );
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomerLiveUpdateAudit {
    received_watermark: String,
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
            DestinationRuntimeMode::SingleContainer,
            "the honest default E2E path must run the destination runtime in one container",
        );
        assert_eq!(
            self.destination_runtime.container_count,
            1,
            "the honest default E2E path must use exactly one destination runner container",
        );
        assert_eq!(
            self.destination_runtime.runner_entrypoint_json.as_deref(),
            Some(EXPECTED_RUNNER_ENTRYPOINT_JSON),
            "the honest default E2E path must boot the production runner image entrypoint",
        );
        assert_eq!(
            self.destination_runtime.destination_connection_host,
            "postgres",
            "the honest default E2E path must apply into PostgreSQL from inside the Docker network",
        );
        assert_eq!(
            self.destination_runtime.destination_connection_port,
            5432,
            "the honest default E2E path must apply into PostgreSQL on the container-network port",
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
            self.cockroach.image,
            EXPECTED_COCKROACH_IMAGE,
            "the honest default E2E path must use the real CockroachDB container image",
        );
        assert!(
            !self.destination_role.is_superuser,
            "the destination runtime role must not be superuser: {:?}",
            self.destination_role,
        );
        if let Some(postgres_apply_client_addr) =
            self.destination_runtime.postgres_apply_client_addr.as_deref()
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct VerifyCommandAudit {
    source_url: String,
    target_url: String,
    schema_filter: String,
    table_filter: String,
    allow_tls_mode_disable: bool,
}

impl VerifyCommandAudit {
    fn from_log(log_path: &Path) -> Self {
        let log = fs::read_to_string(log_path).unwrap_or_else(|error| {
            panic!(
                "verify wrapper log `{}` should be readable: {error}",
                log_path.display()
            )
        });
        let command =
            parse_last_logged_command(&log).expect("verify wrapper log should contain one command");
        Self::from_args(&command)
    }

    fn from_args(args: &[String]) -> Self {
        assert!(
            args.first().map(String::as_str) == Some("verify"),
            "molt wrapper should be invoked with `verify`: {args:?}",
        );

        Self {
            source_url: required_flag_value(args, "--source"),
            target_url: required_flag_value(args, "--target"),
            schema_filter: required_flag_value(args, "--schema-filter"),
            table_filter: required_flag_value(args, "--table-filter"),
            allow_tls_mode_disable: args.iter().any(|arg| arg == "--allow-tls-mode-disable"),
        }
    }

    fn target_database(&self) -> &str {
        self.target_url
            .rsplit('/')
            .next()
            .expect("target URL should contain a database")
    }
}

fn parse_last_logged_command(log: &str) -> Option<Vec<String>> {
    let mut commands = Vec::new();
    let mut current = Vec::new();

    for line in log.lines() {
        if line == "END" {
            if !current.is_empty() {
                commands.push(std::mem::take(&mut current));
            }
            continue;
        }

        let argument = line.strip_prefix("ARG\t").unwrap_or_else(|| {
            panic!("verify wrapper log line should start with `ARG\\t`: {line}")
        });
        current.push(argument.to_owned());
    }

    if !current.is_empty() {
        commands.push(current);
    }

    commands.pop()
}

fn required_flag_value(args: &[String], flag: &str) -> String {
    let position = args
        .iter()
        .position(|arg| arg == flag)
        .unwrap_or_else(|| panic!("verify command should include `{flag}`: {args:?}"));
    args.get(position + 1)
        .unwrap_or_else(|| {
            panic!("verify command should include a value after `{flag}`: {args:?}")
        })
        .clone()
}

fn expected_schema_filter(expected_tables: &[&str]) -> String {
    expected_tables
        .iter()
        .map(|table| split_table_reference(table).0.to_owned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("|")
}

fn expected_table_filter(expected_tables: &[&str]) -> String {
    expected_tables
        .iter()
        .map(|table| split_table_reference(table).1.to_owned())
        .collect::<Vec<_>>()
        .join("|")
}

fn split_table_reference(table: &str) -> (&str, &str) {
    table.split_once('.').unwrap_or_else(|| {
        panic!("mapped table should include a schema and table name: {table}")
    })
}
