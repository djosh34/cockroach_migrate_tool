#![allow(dead_code)]

use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Mutex, MutexGuard, OnceLock},
    thread,
    time::Duration,
};

pub struct TestPostgres {
    _data_dir: tempfile::TempDir,
    process: Child,
    port: u16,
    _suite_guard: MutexGuard<'static, ()>,
}

impl TestPostgres {
    pub fn start() -> Self {
        let suite_guard = bootstrap_suite_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for _ in 0..10 {
            let data_dir = tempfile::tempdir().expect("postgres data dir should be created");
            let port = pick_unused_port();

            run_command(
                Command::new("initdb")
                    .args([
                        "--auth-local=trust",
                        "--auth-host=trust",
                        "--username=postgres",
                        "--pgdata",
                    ])
                    .arg(data_dir.path()),
                "initdb",
            );

            let mut process = Command::new("postgres")
                .args(["-D"])
                .arg(data_dir.path())
                .args([
                    "-F",
                    "-h",
                    "127.0.0.1",
                    "-k",
                    &data_dir.path().display().to_string(),
                    "-p",
                    &port.to_string(),
                    "-c",
                    "logging_collector=off",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("postgres should start");

            if Self::wait_until_ready(&mut process, port, data_dir.path()) {
                return Self {
                    _data_dir: data_dir,
                    process,
                    port,
                    _suite_guard: suite_guard,
                };
            }
        }

        panic!("postgres test cluster could not claim a stable TCP port");
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn exec(&self, database: &str, sql: &str) {
        self.exec_as("postgres", database, sql);
    }

    pub fn exec_as(&self, user: &str, database: &str, sql: &str) {
        run_command(
            Command::new("psql").env("PGPASSWORD", "").args([
                "-h",
                "127.0.0.1",
                "-p",
                &self.port.to_string(),
                "-U",
                user,
                "-d",
                database,
                "-v",
                "ON_ERROR_STOP=1",
                "-c",
                sql,
            ]),
            "psql",
        );
    }

    pub fn query(&self, database: &str, sql: &str) -> String {
        let output = Command::new("psql")
            .env("PGPASSWORD", "")
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &self.port.to_string(),
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
            ])
            .output()
            .expect("psql query should start");

        assert!(
            output.status.success(),
            "psql query failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        String::from_utf8(output.stdout)
            .expect("psql query output should be utf-8")
            .trim()
            .to_owned()
    }

    pub fn write_runner_config(&self, path: &Path, bind_port: u16) {
        self.write_runner_config_with_tables(path, bind_port, &["public.customers"]);
    }

    pub fn write_http_runner_config(&self, path: &Path, bind_port: u16) {
        self.write_http_runner_config_with_tables(path, bind_port, &["public.customers"]);
    }

    pub fn write_explicit_https_runner_config(&self, path: &Path, bind_port: u16) {
        let tables_yaml = "        - public.customers";

        fs::write(
            path,
            format!(
                r#"webhook:
  bind_addr: 127.0.0.1:{bind_port}
  mode: https
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
{tables_yaml}
    destination:
      host: 127.0.0.1
      port: {port}
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
                bind_port = bind_port,
                cert_path = fixture_path("certs/server.crt").display(),
                key_path = fixture_path("certs/server.key").display(),
                tables_yaml = tables_yaml,
                port = self.port,
            ),
        )
        .expect("runner config should be written");
    }

    pub fn write_mtls_runner_config(&self, path: &Path, bind_port: u16) {
        let tables_yaml = "        - public.customers";

        fs::write(
            path,
            format!(
                r#"webhook:
  bind_addr: 127.0.0.1:{bind_port}
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
    client_ca_path: {client_ca_path}
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
{tables_yaml}
    destination:
      host: 127.0.0.1
      port: {port}
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
                bind_port = bind_port,
                cert_path = investigation_cert("server.crt").display(),
                key_path = investigation_cert("server.key").display(),
                client_ca_path = investigation_cert("ca.crt").display(),
                tables_yaml = tables_yaml,
                port = self.port,
            ),
        )
        .expect("runner mtls config should be written");
    }

    pub fn write_runner_config_with_tables(&self, path: &Path, bind_port: u16, tables: &[&str]) {
        let tables_yaml = tables
            .iter()
            .map(|table| format!("        - {table}"))
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(
            path,
            format!(
                r#"webhook:
  bind_addr: 127.0.0.1:{bind_port}
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
{tables_yaml}
    destination:
      host: 127.0.0.1
      port: {port}
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
                bind_port = bind_port,
                cert_path = fixture_path("certs/server.crt").display(),
                key_path = fixture_path("certs/server.key").display(),
                tables_yaml = tables_yaml,
                port = self.port,
            ),
        )
        .expect("runner config should be written");
    }

    pub fn write_http_runner_config_with_tables(
        &self,
        path: &Path,
        bind_port: u16,
        tables: &[&str],
    ) {
        let tables_yaml = tables
            .iter()
            .map(|table| format!("        - {table}"))
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(
            path,
            format!(
                r#"webhook:
  bind_addr: 127.0.0.1:{bind_port}
  mode: http
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
{tables_yaml}
    destination:
      host: 127.0.0.1
      port: {port}
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
                bind_port = bind_port,
                tables_yaml = tables_yaml,
                port = self.port,
            ),
        )
        .expect("runner config should be written");
    }

    pub fn write_shared_destination_runner_config(
        &self,
        path: &Path,
        bind_port: u16,
        app_a_tables: &[&str],
        app_b_tables: &[&str],
    ) {
        self.write_shared_destination_runner_config_with_credentials(
            path,
            bind_port,
            app_a_tables,
            ("migration_user_shared", "runner-secret-shared"),
            app_b_tables,
            ("migration_user_shared", "runner-secret-shared"),
        );
    }

    pub fn write_shared_destination_runner_config_with_credentials(
        &self,
        path: &Path,
        bind_port: u16,
        app_a_tables: &[&str],
        app_a_credentials: (&str, &str),
        app_b_tables: &[&str],
        app_b_credentials: (&str, &str),
    ) {
        let app_a_tables_yaml = app_a_tables
            .iter()
            .map(|table| format!("        - {table}"))
            .collect::<Vec<_>>()
            .join("\n");
        let app_b_tables_yaml = app_b_tables
            .iter()
            .map(|table| format!("        - {table}"))
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(
            path,
            format!(
                r#"webhook:
  bind_addr: 127.0.0.1:{bind_port}
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
{app_a_tables_yaml}
    destination:
      host: 127.0.0.1
      port: {port}
      database: shared_app
      user: {app_a_user}
      password: {app_a_password}
  - id: app-b
    source:
      database: demo_b
      tables:
{app_b_tables_yaml}
    destination:
      host: 127.0.0.1
      port: {port}
      database: shared_app
      user: {app_b_user}
      password: {app_b_password}
"#,
                bind_port = bind_port,
                cert_path = fixture_path("certs/server.crt").display(),
                key_path = fixture_path("certs/server.key").display(),
                app_a_tables_yaml = app_a_tables_yaml,
                app_b_tables_yaml = app_b_tables_yaml,
                app_a_user = app_a_credentials.0,
                app_a_password = app_a_credentials.1,
                app_b_user = app_b_credentials.0,
                app_b_password = app_b_credentials.1,
                port = self.port,
            ),
        )
        .expect("runner config should be written");
    }

    fn wait_until_ready(process: &mut Child, port: u16, expected_data_dir: &Path) -> bool {
        for _ in 0..50 {
            if process
                .try_wait()
                .expect("postgres process status should be readable")
                .is_some()
            {
                return false;
            }

            let status = Command::new("pg_isready")
                .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-U", "postgres"])
                .status()
                .expect("pg_isready should start");
            if status.success() {
                return Self::server_data_directory_matches_expected(port, expected_data_dir);
            }
            thread::sleep(Duration::from_millis(100));
        }

        false
    }

    fn server_data_directory_matches_expected(port: u16, expected_data_dir: &Path) -> bool {
        let output = Command::new("psql")
            .env("PGPASSWORD", "")
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &port.to_string(),
                "-U",
                "postgres",
                "-d",
                "postgres",
                "-t",
                "-A",
                "-c",
                "SHOW data_directory;",
            ])
            .output()
            .expect("psql should confirm the postgres data directory");

        output.status.success()
            && String::from_utf8(output.stdout)
                .expect("postgres data directory should be utf-8")
                .trim()
                == expected_data_dir.display().to_string()
    }
}

impl Drop for TestPostgres {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

fn bootstrap_suite_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn investigation_cert(name: &str) -> PathBuf {
    repo_root()
        .join("investigations")
        .join("cockroach-webhook-cdc")
        .join("certs")
        .join(name)
}

fn pick_unused_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("ephemeral port should bind")
        .local_addr()
        .expect("bound socket should have a local address")
        .port()
}

fn run_command(command: &mut Command, context: &str) {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{context} should start: {error}"));
    assert!(
        output.status.success(),
        "{context} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
