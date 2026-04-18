use std::{
    fs,
    io::Read,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use assert_cmd::Command as AssertCommand;
use predicates::prelude::predicate;
use reqwest::{Certificate, blocking::Client};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
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

struct TestPostgres {
    _data_dir: tempfile::TempDir,
    process: Child,
    port: u16,
}

impl TestPostgres {
    fn start() -> Self {
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

        let process = Command::new("postgres")
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

        let instance = Self {
            _data_dir: data_dir,
            process,
            port,
        };
        instance.wait_until_ready();
        instance
    }

    fn exec(&self, database: &str, sql: &str) {
        self.exec_as("postgres", database, sql);
    }

    fn exec_as(&self, user: &str, database: &str, sql: &str) {
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

    fn write_runner_config(
        &self,
        path: &Path,
        bind_port: u16,
        report_dir: &Path,
        molt_command: &Path,
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
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
reconcile:
  interval_secs: 30
verify:
  molt:
    command: {molt_command}
    report_dir: {report_dir}
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
{tables_yaml}
    destination:
      connection:
        host: 127.0.0.1
        port: {port}
        database: app_a
        user: migration_user_a
        password: runner-secret-a
"#,
                bind_port = bind_port,
                cert_path = fixture_path("certs/server.crt").display(),
                key_path = fixture_path("certs/server.key").display(),
                molt_command = molt_command.display(),
                report_dir = report_dir.display(),
                tables_yaml = tables_yaml,
                port = self.port,
            ),
        )
        .expect("runner config should be written");
    }

    fn wait_until_ready(&self) {
        for _ in 0..50 {
            let status = Command::new("pg_isready")
                .args([
                    "-h",
                    "127.0.0.1",
                    "-p",
                    &self.port.to_string(),
                    "-U",
                    "postgres",
                ])
                .status()
                .expect("pg_isready should start");
            if status.success() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }

        panic!("postgres did not become ready on port {}", self.port);
    }
}

impl Drop for TestPostgres {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

struct RunnerProcess {
    child: Child,
}

impl RunnerProcess {
    fn start(config_path: &Path) -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_runner"))
            .args(["run", "--config"])
            .arg(config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("runner should start");
        Self { child }
    }

    fn assert_healthy(&mut self, url: &str, client: &Client) {
        for _ in 0..50 {
            if let Some(status) = self
                .child
                .try_wait()
                .expect("runner child status should be readable")
            {
                let mut stdout = String::new();
                let mut stderr = String::new();
                self.child
                    .stdout
                    .as_mut()
                    .expect("runner stdout pipe should exist")
                    .read_to_string(&mut stdout)
                    .expect("runner stdout should be readable");
                self.child
                    .stderr
                    .as_mut()
                    .expect("runner stderr pipe should exist")
                    .read_to_string(&mut stderr)
                    .expect("runner stderr should be readable");
                panic!(
                    "runner exited before serving healthz with status {status}\nstdout:\n{stdout}\nstderr:\n{stderr}"
                );
            }

            match client.get(url).send() {
                Ok(response) if response.status().is_success() => return,
                Ok(_) | Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        }

        panic!("runner did not serve healthz at {url}");
    }
}

impl Drop for RunnerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn https_client() -> Client {
    let certificate = Certificate::from_pem(
        &fs::read(fixture_path("certs/server.crt")).expect("server certificate should be readable"),
    )
    .expect("server certificate should parse");

    Client::builder()
        .add_root_certificate(certificate)
        .build()
        .expect("https client should build")
}

fn write_script(path: &Path, contents: &str) {
    fs::write(path, contents).expect("script fixture should be written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)
            .expect("script fixture metadata should exist")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("script fixture should be executable");
    }
}

#[test]
fn cutover_readiness_reports_ready_when_watermarks_drain_and_verify_matches() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
         CREATE TABLE public.orders (id bigint PRIMARY KEY, total_cents bigint NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let report_dir = temp_dir.path().join("reports");
    let script_path = temp_dir.path().join("fake-molt.sh");
    let config_path = temp_dir.path().join("runner.yml");
    write_script(
        &script_path,
        r#"#!/usr/bin/env bash
set -euo pipefail

cat <<'EOF'
{"type":"summary","table_schema":"public","table_name":"customers","num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_column_mismatch":0}
{"type":"summary","table_schema":"public","table_name":"orders","num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_column_mismatch":0}
{"level":"info","message":"verification complete"}
EOF
"#,
    );
    postgres.write_runner_config(
        &config_path,
        bind_port,
        &report_dir,
        &script_path,
        &["public.customers", "public.orders"],
    );

    {
        let mut runner = RunnerProcess::start(&config_path);
        runner.assert_healthy(
            &format!("https://localhost:{bind_port}/healthz"),
            &https_client(),
        );
    }

    postgres.exec(
        "app_a",
        "UPDATE _cockroach_migration_tool.stream_state
         SET latest_received_resolved_watermark = '1776526353000000000.0000000000',
             latest_reconciled_resolved_watermark = '1776526353000000000.0000000000'
         WHERE mapping_id = 'app-a';
         UPDATE _cockroach_migration_tool.table_sync_state
         SET last_successful_sync_watermark = '1776526353000000000.0000000000',
             last_error = NULL
         WHERE mapping_id = 'app-a';",
    );

    let mut command = AssertCommand::cargo_bin("runner").expect("runner binary should exist");
    command
        .args(["cutover-readiness", "--config"])
        .arg(&config_path)
        .args(["--mapping", "app-a", "--source-url"])
        .arg("postgres://root@127.0.0.1:26257/demo_a?sslmode=disable")
        .assert()
        .success()
        .stdout(predicate::str::contains("cutover readiness"))
        .stdout(predicate::str::contains("mapping=app-a"))
        .stdout(predicate::str::contains("ready=true"))
        .stdout(predicate::str::contains("verification=matched"));
}

#[test]
fn cutover_readiness_reports_watermark_lag_and_skips_verify() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
         CREATE TABLE public.orders (id bigint PRIMARY KEY, total_cents bigint NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let report_dir = temp_dir.path().join("reports");
    let verify_marker = temp_dir.path().join("verify-ran.txt");
    let script_path = temp_dir.path().join("fake-molt.sh");
    let config_path = temp_dir.path().join("runner.yml");
    write_script(
        &script_path,
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

touch "{}"
exit 99
"#,
            verify_marker.display()
        ),
    );
    postgres.write_runner_config(
        &config_path,
        bind_port,
        &report_dir,
        &script_path,
        &["public.customers", "public.orders"],
    );

    {
        let mut runner = RunnerProcess::start(&config_path);
        runner.assert_healthy(
            &format!("https://localhost:{bind_port}/healthz"),
            &https_client(),
        );
    }

    postgres.exec(
        "app_a",
        "UPDATE _cockroach_migration_tool.stream_state
         SET latest_received_resolved_watermark = '1776526353000000000.0000000001',
             latest_reconciled_resolved_watermark = '1776526353000000000.0000000000'
         WHERE mapping_id = 'app-a';
         UPDATE _cockroach_migration_tool.table_sync_state
         SET last_successful_sync_watermark = '1776526353000000000.0000000000',
             last_error = NULL
         WHERE mapping_id = 'app-a';",
    );

    let mut command = AssertCommand::cargo_bin("runner").expect("runner binary should exist");
    command
        .args(["cutover-readiness", "--config"])
        .arg(&config_path)
        .args(["--mapping", "app-a", "--source-url"])
        .arg("postgres://root@127.0.0.1:26257/demo_a?sslmode=disable")
        .assert()
        .success()
        .stdout(predicate::str::contains("ready=false"))
        .stdout(predicate::str::contains("verification=skipped"))
        .stdout(predicate::str::contains(
            "cdc/reconcile has not drained yet",
        ));

    assert!(
        !verify_marker.exists(),
        "verify must be skipped while watermarks still lag"
    );
}

#[test]
fn cutover_readiness_reports_table_drain_errors_and_skips_verify() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
         CREATE TABLE public.orders (id bigint PRIMARY KEY, total_cents bigint NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let report_dir = temp_dir.path().join("reports");
    let verify_marker = temp_dir.path().join("verify-ran.txt");
    let script_path = temp_dir.path().join("fake-molt.sh");
    let config_path = temp_dir.path().join("runner.yml");
    write_script(
        &script_path,
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

touch "{}"
exit 99
"#,
            verify_marker.display()
        ),
    );
    postgres.write_runner_config(
        &config_path,
        bind_port,
        &report_dir,
        &script_path,
        &["public.customers", "public.orders"],
    );

    {
        let mut runner = RunnerProcess::start(&config_path);
        runner.assert_healthy(
            &format!("https://localhost:{bind_port}/healthz"),
            &https_client(),
        );
    }

    postgres.exec(
        "app_a",
        "UPDATE _cockroach_migration_tool.stream_state
         SET latest_received_resolved_watermark = '1776526353000000000.0000000000',
             latest_reconciled_resolved_watermark = '1776526353000000000.0000000000'
         WHERE mapping_id = 'app-a';
         UPDATE _cockroach_migration_tool.table_sync_state
         SET last_successful_sync_watermark = '1776526353000000000.0000000000',
             last_error = NULL
         WHERE mapping_id = 'app-a';
         UPDATE _cockroach_migration_tool.table_sync_state
         SET last_error = 'reconcile upsert failed for public.orders: duplicate key'
         WHERE mapping_id = 'app-a'
           AND source_table_name = 'public.orders';",
    );

    let mut command = AssertCommand::cargo_bin("runner").expect("runner binary should exist");
    command
        .args(["cutover-readiness", "--config"])
        .arg(&config_path)
        .args(["--mapping", "app-a", "--source-url"])
        .arg("postgres://root@127.0.0.1:26257/demo_a?sslmode=disable")
        .assert()
        .success()
        .stdout(predicate::str::contains("ready=false"))
        .stdout(predicate::str::contains("verification=skipped"))
        .stdout(predicate::str::contains("table drain is incomplete"))
        .stdout(predicate::str::contains("public.orders"))
        .stdout(predicate::str::contains("duplicate key"));

    assert!(
        !verify_marker.exists(),
        "verify must be skipped while any selected table still has a stored reconcile error"
    );
}

#[test]
fn cutover_readiness_reports_verify_mismatches_after_drain_reaches_zero() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
         CREATE TABLE public.orders (id bigint PRIMARY KEY, total_cents bigint NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let report_dir = temp_dir.path().join("reports");
    let verify_marker = temp_dir.path().join("verify-ran.txt");
    let script_path = temp_dir.path().join("fake-molt.sh");
    let config_path = temp_dir.path().join("runner.yml");
    write_script(
        &script_path,
        &format!(
            r#"#!/usr/bin/env bash
set -euo pipefail

touch "{}"

cat <<'EOF'
{{"type":"summary","table_schema":"public","table_name":"customers","num_missing":0,"num_mismatch":1,"num_extraneous":0,"num_column_mismatch":0}}
{{"type":"summary","table_schema":"public","table_name":"orders","num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_column_mismatch":0}}
{{"level":"info","message":"verification complete"}}
EOF
"#,
            verify_marker.display()
        ),
    );
    postgres.write_runner_config(
        &config_path,
        bind_port,
        &report_dir,
        &script_path,
        &["public.customers", "public.orders"],
    );

    {
        let mut runner = RunnerProcess::start(&config_path);
        runner.assert_healthy(
            &format!("https://localhost:{bind_port}/healthz"),
            &https_client(),
        );
    }

    postgres.exec(
        "app_a",
        "UPDATE _cockroach_migration_tool.stream_state
         SET latest_received_resolved_watermark = '1776526353000000000.0000000000',
             latest_reconciled_resolved_watermark = '1776526353000000000.0000000000'
         WHERE mapping_id = 'app-a';
         UPDATE _cockroach_migration_tool.table_sync_state
         SET last_successful_sync_watermark = '1776526353000000000.0000000000',
             last_error = NULL
         WHERE mapping_id = 'app-a';",
    );

    let mut command = AssertCommand::cargo_bin("runner").expect("runner binary should exist");
    command
        .args(["cutover-readiness", "--config"])
        .arg(&config_path)
        .args(["--mapping", "app-a", "--source-url"])
        .arg("postgres://root@127.0.0.1:26257/demo_a?sslmode=disable")
        .assert()
        .success()
        .stdout(predicate::str::contains("ready=false"))
        .stdout(predicate::str::contains("verification=mismatch"))
        .stdout(predicate::str::contains("verification found mismatches"))
        .stdout(predicate::str::contains("customers"))
        .stdout(predicate::str::contains("num_mismatch=1"));

    assert!(
        verify_marker.exists(),
        "verify must run once the drain checks have reached zero"
    );
}

#[test]
fn cutover_readiness_fails_loudly_when_selected_table_tracking_state_is_missing() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
         CREATE TABLE public.orders (id bigint PRIMARY KEY, total_cents bigint NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let report_dir = temp_dir.path().join("reports");
    let script_path = temp_dir.path().join("fake-molt.sh");
    let config_path = temp_dir.path().join("runner.yml");
    write_script(
        &script_path,
        r#"#!/usr/bin/env bash
set -euo pipefail

exit 99
"#,
    );
    postgres.write_runner_config(
        &config_path,
        bind_port,
        &report_dir,
        &script_path,
        &["public.customers", "public.orders"],
    );

    {
        let mut runner = RunnerProcess::start(&config_path);
        runner.assert_healthy(
            &format!("https://localhost:{bind_port}/healthz"),
            &https_client(),
        );
    }

    postgres.exec(
        "app_a",
        "DELETE FROM _cockroach_migration_tool.table_sync_state
         WHERE mapping_id = 'app-a'
           AND source_table_name = 'public.orders';",
    );

    let mut command = AssertCommand::cargo_bin("runner").expect("runner binary should exist");
    command
        .args(["cutover-readiness", "--config"])
        .arg(&config_path)
        .args(["--mapping", "app-a", "--source-url"])
        .arg("postgres://root@127.0.0.1:26257/demo_a?sslmode=disable")
        .assert()
        .failure()
        .stderr(predicate::str::contains("helper bootstrap is incomplete"))
        .stderr(predicate::str::contains("public.orders"));
}
