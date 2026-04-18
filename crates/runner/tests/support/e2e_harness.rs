use std::{
    cell::RefCell,
    env,
    ffi::OsString,
    fs::{self, File},
    io,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::{Certificate, blocking::Client};
use tempfile::TempDir;

const COCKROACH_IMAGE: &str = "cockroachdb/cockroach:v26.1.2";
const POSTGRES_IMAGE: &str = "postgres:16";
const MOLT_IMAGE: &str =
    "cockroachdb/molt@sha256:abe3c90bc42556ad6713cba207b971e6d55dbd54211b53cfcf27cdc14d49e358";

pub struct DefaultBootstrapHarness {
    docker: DockerEnvironment,
    temp_dir: TempDir,
    runner_port: u16,
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

impl DefaultBootstrapHarness {
    pub fn start() -> Self {
        let docker = DockerEnvironment::new();
        docker.create_network();
        docker.start_cockroach();
        docker.start_postgres();
        docker.wait_for_cockroach();
        docker.wait_for_postgres();
        docker.prepare_default_source_schema_and_seed();
        docker.prepare_default_destination_schema();

        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let runner_port = pick_unused_port();
        let wrapper_bin_dir = temp_dir.path().join("bin");
        let report_dir = temp_dir.path().join("reports");
        fs::create_dir_all(&wrapper_bin_dir).expect("wrapper bin dir should be created");
        fs::create_dir_all(&report_dir).expect("report dir should be created");

        let harness = Self {
            docker,
            temp_dir,
            runner_port,
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
        harness.materialize()
    }

    pub fn bootstrap_default_migration(&self) {
        self.start_runner_process();
        wait_for_runner_health(
            &https_client(&investigation_ca_cert_path()),
            self.runner_port,
            || self.runner_logs(),
        );
        self.render_source_bootstrap_script();
        self.execute_bootstrap_script();
    }

    pub fn wait_for_destination_customers(&self, expected: &str) {
        for _ in 0..120 {
            self.assert_runner_alive();
            if self
                .docker
                .query_postgres_customers("app_a")
                .trim()
                .eq(expected)
            {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "destination customers did not converge to `{expected}`\nsource={}\ndestination={}\nrunner stderr:\n{}",
            self.docker.query_cockroach_customers("demo_a").trim(),
            self.docker.query_postgres_customers("app_a").trim(),
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
        assert!(
            commands.iter().any(|command| command.contains(
                "CREATE CHANGEFEED FOR TABLE public.customers"
            )),
            "bootstrap should create the changefeed explicitly: {log}"
        );
    }

    pub fn assert_helper_shadow_customers(&self, expected_rows: usize) {
        let tables = self.helper_tables("app_a");
        assert!(
            tables.contains("app-a__public__customers"),
            "helper bootstrap should create the customers shadow table: {tables}"
        );
        let row_count = self.docker.exec_psql(
            "app_a",
            "SELECT count(*)::text
             FROM _cockroach_migration_tool.\"app-a__public__customers\";",
        );
        assert_eq!(
            row_count.trim(),
            expected_rows.to_string(),
            "helper shadow table should contain the initial scan rows"
        );
    }

    pub fn verify_default_migration(&self) {
        self.assert_runner_alive();
        let output = run_command_capture(
            Command::new(env!("CARGO_BIN_EXE_runner"))
                .args(["verify", "--config"])
                .arg(&self.runner_config_path)
                .args([
                    "--mapping",
                    "app-a",
                    "--source-url",
                    "postgresql://root@127.0.0.1:26257/demo_a?sslmode=disable",
                    "--allow-tls-mode-disable",
                ]),
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
            output.contains("tables=public.customers"),
            "verify output should mention the real migrated table only: {output}"
        );
    }

    fn helper_tables(&self, database: &str) -> String {
        self.docker.exec_psql(
            database,
            "SELECT string_agg(table_name, ',' ORDER BY table_name)
             FROM information_schema.tables
             WHERE table_schema = '_cockroach_migration_tool';",
        )
    }

    fn materialize(mut self) -> Self {
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
        fs::write(
            &self.runner_config_path,
            format!(
                r#"webhook:
  bind_addr: 0.0.0.0:{runner_port}
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
reconcile:
  interval_secs: 1
verify:
  molt:
    command: {molt_command}
    report_dir: {report_dir}
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
    destination:
      connection:
        host: 127.0.0.1
        port: {postgres_port}
        database: app_a
        user: migration_user_a
        password: runner-secret-a
"#,
                runner_port = self.runner_port,
                cert_path = investigation_server_cert_path().display(),
                key_path = investigation_server_key_path().display(),
                molt_command = self.wrapper_bin_dir.join("molt").display(),
                report_dir = self.report_dir.display(),
                postgres_port = self.docker.postgres_host_port,
            ),
        )
        .expect("runner config should be written");
    }

    fn write_source_bootstrap_config(&self) {
        fs::write(
            &self.source_bootstrap_config_path,
            format!(
                r#"cockroach:
  url: postgresql://root@127.0.0.1:26257/defaultdb?sslmode=disable
webhook:
  base_url: https://host.docker.internal:{runner_port}
  ca_cert_path: {ca_cert_path}
  resolved: 1s
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
"#,
                runner_port = self.runner_port,
                ca_cert_path = investigation_ca_cert_path().display(),
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
}

impl Drop for DefaultBootstrapHarness {
    fn drop(&mut self) {
        if let Some(child) = self.runner_process.borrow_mut().as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

struct DockerEnvironment {
    network_name: String,
    cockroach_container: String,
    postgres_container: String,
    postgres_host_port: u16,
}

impl DockerEnvironment {
    fn new() -> Self {
        let suffix = unique_suffix();
        Self {
            network_name: format!("cockroach-migrate-runner-net-{suffix}"),
            cockroach_container: format!("cockroach-migrate-cockroach-{suffix}"),
            postgres_container: format!("cockroach-migrate-postgres-{suffix}"),
            postgres_host_port: pick_unused_port(),
        }
    }

    fn create_network(&self) {
        run_command_capture(
            Command::new("docker").args(["network", "create", &self.network_name]),
            "docker network create",
        );
    }

    fn start_cockroach(&self) {
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

    fn start_postgres(&self) {
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

    fn wait_for_cockroach(&self) {
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

    fn wait_for_postgres(&self) {
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

    fn prepare_default_source_schema_and_seed(&self) {
        self.exec_cockroach_sql(
            "CREATE DATABASE demo_a;
             USE demo_a;
             CREATE TABLE public.customers (
                 id INT8 PRIMARY KEY,
                 email STRING NOT NULL
             );
             INSERT INTO public.customers (id, email) VALUES
                 (1, 'alice@example.com'),
                 (2, 'bob@example.com');",
        );
    }

    fn prepare_default_destination_schema(&self) {
        self.exec_psql(
            "postgres",
            "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
        );
        self.exec_psql(
            "postgres",
            "CREATE DATABASE app_a OWNER migration_user_a;",
        );
        self.exec_psql(
            "app_a",
            "SET ROLE migration_user_a;
             CREATE TABLE public.customers (
                 id bigint PRIMARY KEY,
                 email text NOT NULL
             );",
        );
    }

    fn exec_cockroach_sql(&self, sql: &str) -> String {
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

    fn exec_psql(&self, database: &str, sql: &str) -> String {
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

    fn query_postgres_customers(&self, database: &str) -> String {
        self.exec_psql(
            database,
            "SELECT COALESCE(
                 string_agg(id::text || ':' || email, ',' ORDER BY id),
                 '<empty>'
             )
             FROM public.customers;",
        )
    }

    fn query_cockroach_customers(&self, database: &str) -> String {
        self.exec_cockroach_sql(&format!(
            "USE {database};
             SELECT COALESCE(
                 string_agg(CAST(id AS STRING) || ':' || email, ',' ORDER BY id),
                 '<empty>'
             )
             FROM public.customers;"
        ))
        .lines()
        .last()
        .unwrap_or_default()
        .trim()
        .to_owned()
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

fn investigation_ca_cert_path() -> PathBuf {
    repo_root()
        .join("investigations")
        .join("cockroach-webhook-cdc")
        .join("certs")
        .join("ca.crt")
}

fn investigation_server_cert_path() -> PathBuf {
    repo_root()
        .join("investigations")
        .join("cockroach-webhook-cdc")
        .join("certs")
        .join("server.crt")
}

fn investigation_server_key_path() -> PathBuf {
    repo_root()
        .join("investigations")
        .join("cockroach-webhook-cdc")
        .join("certs")
        .join("server.key")
}

fn source_bootstrap_binary_path() -> PathBuf {
    repo_root().join("target").join("debug").join("source-bootstrap")
}

fn ensure_source_bootstrap_binary() {
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

fn pick_unused_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("ephemeral port should bind")
        .local_addr()
        .expect("bound socket should have a local address")
        .port()
}

fn write_cockroach_wrapper_script(path: &Path, log_path: &Path, container_name: &str) {
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

fn write_molt_wrapper_script(path: &Path, cockroach_container: &str) {
    fs::write(
        path,
        format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nargs=()\nrewrite_target=0\nfor arg in \"$@\"; do\n  if [[ \"$rewrite_target\" == 1 ]]; then\n    args+=(\"postgresql://migration_user_a:runner-secret-a@postgres:5432/app_a\")\n    rewrite_target=0\n    continue\n  fi\n  args+=(\"$arg\")\n  if [[ \"$arg\" == \"--target\" ]]; then\n    rewrite_target=1\n  fi\ndone\nexec docker run --rm --network container:{cockroach_container} {image} \"${{args[@]}}\"\n",
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

fn https_client(certificate_path: &Path) -> Client {
    let certificate = Certificate::from_pem(
        &fs::read(certificate_path).expect("certificate should be readable"),
    )
    .expect("certificate should parse");

    Client::builder()
        .add_root_certificate(certificate)
        .build()
        .expect("https client should build")
}

fn wait_for_runner_health<F>(client: &Client, port: u16, logs: F)
where
    F: Fn() -> String,
{
    for _ in 0..60 {
        match client.get(format!("https://localhost:{port}/healthz")).send() {
            Ok(response) if response.status().is_success() => return,
            Ok(_) | Err(_) => thread::sleep(Duration::from_secs(1)),
        }
    }

    panic!(
        "runner did not become healthy on https://localhost:{port}/healthz\n{}",
        logs()
    );
}

fn prepend_path(bin_dir: &Path) -> OsString {
    let mut path = OsString::new();
    path.push(bin_dir.as_os_str());
    path.push(":");
    path.push(env::var_os("PATH").unwrap_or_default());
    path
}

fn run_command_capture(command: &mut Command, context: &str) -> String {
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

fn read_file(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => panic!("failed to read `{}`: {error}", path.display()),
    }
}

fn shell_quote(path: &Path) -> String {
    shell_quote_text(&path.display().to_string())
}

fn shell_quote_text(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}
