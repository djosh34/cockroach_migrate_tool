use std::{
    cell::RefCell,
    env,
    ffi::OsString,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use reqwest::{Certificate, blocking::Client};
use tempfile::TempDir;

use crate::webhook_chaos_gateway::WebhookChaosGateway;

const COCKROACH_IMAGE: &str = "cockroachdb/cockroach:v26.1.2";
const POSTGRES_IMAGE: &str = "postgres:16";
const MOLT_IMAGE: &str =
    "cockroachdb/molt@sha256:abe3c90bc42556ad6713cba207b971e6d55dbd54211b53cfcf27cdc14d49e358";

#[derive(Clone, Copy)]
pub enum WebhookSinkMode {
    DirectRunner,
    ExternalChaosGateway,
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

pub struct CdcE2eHarness {
    docker: DockerEnvironment,
    config: OwnedHarnessConfig,
    temp_dir: TempDir,
    runner_port: u16,
    webhook_sink_base_url: String,
    webhook_chaos_gateway: Option<WebhookChaosGateway>,
    runner_config_path: PathBuf,
    source_bootstrap_config_path: PathBuf,
    source_bootstrap_script_path: PathBuf,
    wrapper_bin_dir: PathBuf,
    report_dir: PathBuf,
    cockroach_wrapper_log_path: PathBuf,
    runner_stdout_path: PathBuf,
    runner_stderr_path: PathBuf,
    runner_process: RefCell<Option<Child>>,
}

impl CdcE2eHarness {
    pub fn start(config: CdcE2eHarnessConfig<'_>) -> Self {
        Self::start_with_webhook_sink(config, WebhookSinkMode::DirectRunner)
    }

    pub fn start_with_webhook_sink(
        config: CdcE2eHarnessConfig<'_>,
        webhook_sink_mode: WebhookSinkMode,
    ) -> Self {
        let config = OwnedHarnessConfig::from(config);
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
        let report_dir = temp_dir.path().join("reports");
        fs::create_dir_all(&wrapper_bin_dir).expect("wrapper bin dir should be created");
        fs::create_dir_all(&report_dir).expect("report dir should be created");

        let harness = Self {
            docker,
            config,
            temp_dir,
            runner_port,
            webhook_sink_base_url: String::new(),
            webhook_chaos_gateway: None,
            runner_config_path: PathBuf::new(),
            source_bootstrap_config_path: PathBuf::new(),
            source_bootstrap_script_path: PathBuf::new(),
            wrapper_bin_dir,
            report_dir,
            cockroach_wrapper_log_path: PathBuf::new(),
            runner_stdout_path: PathBuf::new(),
            runner_stderr_path: PathBuf::new(),
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
        self.render_source_bootstrap_script();
        self.execute_bootstrap_script();
    }

    pub fn wait_for_destination_query(&self, sql: &str, expected: &str, description: &str) {
        for _ in 0..120 {
            self.assert_runner_alive();
            let actual = self.query_destination(sql);
            if actual.trim() == expected {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "{description} did not converge to `{expected}`\nactual={}\nrunner stderr:\n{}",
            self.query_destination(sql).trim(),
            read_file(&self.runner_stderr_path),
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
            self.assert_runner_alive();
            let actual = self.query_destination(sql);
            assert_eq!(
                actual.trim(),
                expected,
                "{description} changed unexpectedly while it should remain stable\nrunner stderr:\n{}",
                read_file(&self.runner_stderr_path),
            );
            if Instant::now() >= deadline {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    pub fn wait_for_helper_table_row_counts(&self, expectations: &[(&str, usize)]) {
        for _ in 0..120 {
            self.assert_runner_alive();
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
            read_file(&self.runner_stderr_path),
        );
    }

    pub fn wait_for_helper_tables(&self, expected: &str, description: &str) {
        for _ in 0..120 {
            self.assert_runner_alive();
            let actual = self.helper_tables();
            if actual.trim() == expected {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "{description} did not converge to `{expected}`\nactual={}\nrunner stderr:\n{}",
            self.helper_tables().trim(),
            read_file(&self.runner_stderr_path),
        );
    }

    pub fn assert_explicit_source_bootstrap_commands(&self) {
        let log = read_file(&self.cockroach_wrapper_log_path);
        let commands: Vec<_> = log.lines().collect();
        assert_eq!(
            commands.len(),
            3,
            "bootstrap should issue exactly three raw source commands: {log}"
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains("SET CLUSTER SETTING kv.rangefeed.enabled = true;")),
            "bootstrap should enable rangefeeds explicitly: {log}"
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains("SELECT cluster_logical_timestamp();")),
            "bootstrap should capture the start cursor explicitly: {log}"
        );
        let expected_changefeed_fragment = format!(
            "CREATE CHANGEFEED FOR TABLE {}",
            self.config.selected_tables.join(", ")
        );
        assert!(
            commands
                .iter()
                .any(|command| command.contains(&expected_changefeed_fragment)),
            "bootstrap should create the expected changefeed explicitly: {log}"
        );
    }

    pub fn verify_migration(&self) -> String {
        self.assert_runner_alive();
        let output = run_command_capture(
            Command::new(env!("CARGO_BIN_EXE_runner"))
                .args(["verify", "--config"])
                .arg(&self.runner_config_path)
                .args(["--mapping", &self.config.mapping_id, "--source-url"])
                .arg(self.source_url())
                .arg("--allow-tls-mode-disable"),
            "runner verify",
        );
        assert!(
            output.contains("verification"),
            "verify output should include a verification summary: {output}"
        );
        assert!(
            output.contains("verdict=matched"),
            "verify output should report a matched verdict: {output}"
        );
        assert!(
            output.contains(&format!("tables={}", self.config.selected_tables.join(","))),
            "verify output should mention only the real migrated tables: {output}"
        );
        output
    }

    pub fn arm_single_external_http_500_for_request_body(&self, body_substring: &str) {
        self.webhook_chaos_gateway
            .as_ref()
            .expect("chaos gateway should be configured for this harness")
            .arm_single_external_http_500_for_body_substring(body_substring);
    }

    pub fn wait_for_duplicate_gateway_delivery_of_request_body(&self, body_substring: &str) {
        for _ in 0..120 {
            self.assert_runner_alive();
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
            read_file(&self.runner_stderr_path),
        );
    }

    pub fn execute_source_sql(&self, sql: &str) {
        self.docker
            .exec_cockroach_sql(&format!("USE {};\n{sql}", self.config.source_database));
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

    fn materialize(mut self, webhook_sink_mode: WebhookSinkMode) -> Self {
        self.runner_config_path = self.temp_dir.path().join("runner.yml");
        self.source_bootstrap_config_path = self.temp_dir.path().join("source-bootstrap.yml");
        self.source_bootstrap_script_path = self.temp_dir.path().join("bootstrap.sh");
        self.cockroach_wrapper_log_path = self.temp_dir.path().join("cockroach-wrapper.log");
        self.runner_stdout_path = self.temp_dir.path().join("runner.stdout.log");
        self.runner_stderr_path = self.temp_dir.path().join("runner.stderr.log");

        write_cockroach_wrapper_script(
            &self.wrapper_bin_dir.join("cockroach"),
            &self.cockroach_wrapper_log_path,
            &self.docker.cockroach_container,
        );
        write_molt_wrapper_script(
            &self.wrapper_bin_dir.join("molt"),
            &self.docker.cockroach_container,
        );
        match webhook_sink_mode {
            WebhookSinkMode::DirectRunner => {
                self.webhook_sink_base_url =
                    format!("https://host.docker.internal:{}", self.runner_port);
            }
            WebhookSinkMode::ExternalChaosGateway => {
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

        let stdout = File::create(&self.runner_stdout_path).expect("runner stdout log should open");
        let stderr = File::create(&self.runner_stderr_path).expect("runner stderr log should open");
        let child = Command::new(env!("CARGO_BIN_EXE_runner"))
            .args(["run", "--config"])
            .arg(&self.runner_config_path)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("runner process should start");
        *self.runner_process.borrow_mut() = Some(child);
    }

    fn render_source_bootstrap_script(&self) {
        ensure_source_bootstrap_binary();
        let output = run_command_output(
            Command::new(source_bootstrap_binary_path())
                .args(["render-bootstrap-script", "--config"])
                .arg(&self.source_bootstrap_config_path),
            "source-bootstrap render-bootstrap-script",
        );
        fs::write(&self.source_bootstrap_script_path, &output.stdout)
            .expect("bootstrap script should be written");
        make_executable(&self.source_bootstrap_script_path);
    }

    fn execute_bootstrap_script(&self) {
        let path = prepend_path(&self.wrapper_bin_dir);
        run_command_capture(
            Command::new("bash")
                .arg(&self.source_bootstrap_script_path)
                .env("PATH", path),
            "bootstrap shell script",
        );
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
  bind_addr: 0.0.0.0:{runner_port}
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
reconcile:
  interval_secs: {reconcile_interval_secs}
verify:
  molt:
    command: {molt_command}
    report_dir: {report_dir}
mappings:
  - id: {mapping_id}
    source:
      database: {source_database}
      tables:
{selected_tables}
    destination:
      connection:
        host: 127.0.0.1
        port: {postgres_port}
        database: {destination_database}
        user: {destination_user}
        password: {destination_password}
"#,
                runner_port = self.runner_port,
                cert_path = investigation_server_cert_path().display(),
                key_path = investigation_server_key_path().display(),
                molt_command = self.wrapper_bin_dir.join("molt").display(),
                report_dir = self.report_dir.display(),
                mapping_id = self.config.mapping_id,
                source_database = self.config.source_database,
                selected_tables = selected_tables,
                postgres_port = self.docker.postgres_host_port,
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
  url: postgresql://root@127.0.0.1:26257/defaultdb?sslmode=disable
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
                webhook_sink_base_url = self.webhook_sink_base_url,
                ca_cert_path = investigation_ca_cert_path().display(),
                mapping_id = self.config.mapping_id,
                source_database = self.config.source_database,
                selected_tables = selected_tables,
            ),
        )
        .expect("source-bootstrap config should be written");
    }

    fn assert_runner_alive(&self) {
        let mut process = self.runner_process.borrow_mut();
        let Some(child) = process.as_mut() else {
            return;
        };

        if let Some(status) = child
            .try_wait()
            .expect("runner process status should be readable")
        {
            panic!(
                "runner exited early with status {status}\nstdout:\n{}\nstderr:\n{}",
                read_file(&self.runner_stdout_path),
                read_file(&self.runner_stderr_path),
            );
        }
    }

    fn runner_logs(&self) -> String {
        format!(
            "stdout:\n{}\n\nstderr:\n{}",
            read_file(&self.runner_stdout_path),
            read_file(&self.runner_stderr_path),
        )
    }

    fn source_url(&self) -> String {
        format!(
            "postgresql://root@127.0.0.1:26257/{}?sslmode=disable",
            self.config.source_database
        )
    }

    fn helper_table_name(&self, mapped_table: &str) -> String {
        let (schema, table) = split_table_reference(mapped_table);
        format!("{}__{}__{}", self.config.mapping_id, schema, table)
    }
}

impl Drop for CdcE2eHarness {
    fn drop(&mut self) {
        if let Some(child) = self.runner_process.borrow_mut().as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub(crate) struct DockerEnvironment {
    network_name: String,
    pub(crate) cockroach_container: String,
    postgres_container: String,
    pub(crate) postgres_host_port: u16,
}

impl DockerEnvironment {
    pub(crate) fn new() -> Self {
        let suffix = unique_suffix();
        Self {
            network_name: format!("cockroach-migrate-runner-net-{suffix}"),
            cockroach_container: format!("cockroach-migrate-cockroach-{suffix}"),
            postgres_container: format!("cockroach-migrate-postgres-{suffix}"),
            postgres_host_port: pick_unused_port(),
        }
    }

    pub(crate) fn create_network(&self) {
        run_command_capture(
            Command::new("docker").args(["network", "create", &self.network_name]),
            "docker network create",
        );
    }

    pub(crate) fn start_cockroach(&self) {
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
                COCKROACH_IMAGE,
                "start-single-node",
                "--insecure",
                "--listen-addr=localhost:26257",
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
                    "--insecure",
                    "--host=localhost:26257",
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
                "--insecure",
                "--host=localhost:26257",
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
}

impl Drop for DockerEnvironment {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.postgres_container])
            .output();
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.cockroach_container])
            .output();
        let _ = Command::new("docker")
            .args(["network", "rm", &self.network_name])
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

pub(crate) fn source_bootstrap_binary_path() -> PathBuf {
    repo_root()
        .join("target")
        .join("debug")
        .join("source-bootstrap")
}

pub(crate) fn ensure_source_bootstrap_binary() {
    run_command_capture(
        Command::new("cargo").args(["build", "-p", "source-bootstrap"]),
        "cargo build source-bootstrap",
    );
}

fn unique_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos()
        .to_string()
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
            "#!/usr/bin/env bash\nset -euo pipefail\nprintf '%s\\n' \"$*\" >> {log_path}\nexec docker exec {container_name} cockroach \"$@\"\n",
            log_path = shell_quote(log_path),
            container_name = shell_quote_text(container_name),
        ),
    )
    .expect("wrapper script should be written");
    make_executable(path);
}

pub(crate) fn write_molt_wrapper_script(path: &Path, cockroach_container: &str) {
    fs::write(
        path,
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nargs=()\nrewrite_target=0\nfor arg in \"$@\"; do\n  if [[ \"$rewrite_target\" == 1 ]]; then\n    if [[ \"$arg\" != postgresql://*@*/* ]]; then\n      printf 'unexpected --target url for molt wrapper: %s\\n' \"$arg\" >&2\n      exit 1\n    fi\n    target_prefix=\"${{arg%@*}}\"\n    target_database=\"${{arg##*/}}\"\n    args+=(\"${{target_prefix}}@postgres:5432/${{target_database}}\")\n    rewrite_target=0\n    continue\n  fi\n  args+=(\"$arg\")\n  if [[ \"$arg\" == \"--target\" ]]; then\n    rewrite_target=1\n  fi\ndone\nif [[ \"$rewrite_target\" == 1 ]]; then\n  printf 'molt wrapper expected a value after --target\\n' >&2\n  exit 1\nfi\nexec docker run --rm --network container:{cockroach_container} {image} \"${{args[@]}}\"\n",
            cockroach_container = shell_quote_text(cockroach_container),
            image = shell_quote_text(MOLT_IMAGE),
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

pub(crate) fn prepend_path(bin_dir: &Path) -> OsString {
    let mut path = OsString::new();
    path.push(bin_dir.as_os_str());
    path.push(":");
    path.push(env::var_os("PATH").unwrap_or_default());
    path
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

fn shell_quote(path: &Path) -> String {
    shell_quote_text(&path.display().to_string())
}

fn shell_quote_text(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}
