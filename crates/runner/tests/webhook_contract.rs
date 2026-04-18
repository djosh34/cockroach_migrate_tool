use std::{
    fs,
    io::Read,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use reqwest::{Certificate, StatusCode, blocking::Client};

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
            Command::new("initdb").args([
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
        run_command(
            Command::new("psql")
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
                    "-c",
                    sql,
                ]),
            "psql",
        );
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

    fn write_runner_config(&self, path: &Path, bind_port: u16) {
        self.write_runner_config_with_tables(path, bind_port, &["public.customers"]);
    }

    fn write_runner_config_with_tables(&self, path: &Path, bind_port: u16, tables: &[&str]) {
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
    command: molt
    report_dir: /tmp/molt
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
{tables_yaml}
    destination:
      connection:
        host: 127.0.0.1
        port: {postgres_port}
        database: app_a
        user: migration_user_a
        password: runner-secret-a
"#,
                cert_path = fixture_path("certs/server.crt").display(),
                key_path = fixture_path("certs/server.key").display(),
                postgres_port = self.port,
                tables_yaml = tables_yaml,
            ),
        )
        .expect("runner config should be written");
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
                Ok(response) if response.status().is_success() => {
                    let body = response.text().expect("healthz body should be readable");
                    assert_eq!(body, "ok");
                    return;
                }
                Ok(_) | Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        }

        panic!("runner did not serve healthz at {url}");
    }

    fn post(&self, url: &str, client: &Client, body: &str) -> reqwest::blocking::Response {
        client
            .post(url)
            .header("content-type", "application/json")
            .body(body.to_owned())
            .send()
            .expect("ingest request should complete")
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

#[test]
fn run_serves_healthz_over_real_tls_after_bootstrap() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, bind_port);

    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(
        &format!("https://localhost:{bind_port}/healthz"),
        &https_client(),
    );
}

#[test]
fn run_exposes_mapping_scoped_ingest_paths_and_404s_unknown_mapping_ids() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, bind_port);

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    let health_url = format!("https://localhost:{bind_port}/healthz");
    runner.assert_healthy(&health_url, &client);

    let known_mapping_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        "{}",
    );
    assert_ne!(known_mapping_response.status(), StatusCode::NOT_FOUND);

    let unknown_mapping_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/missing"),
        &client,
        "{}",
    );
    assert_eq!(unknown_mapping_response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn run_distinguishes_malformed_json_from_supported_payload_shapes() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, bind_port);

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let malformed = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        "{",
    );
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);

    let row_batch = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers"),
    );
    assert_eq!(row_batch.status(), StatusCode::NOT_IMPLEMENTED);

    let resolved = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        r#"{"resolved":"1776526353000000000.0000000000"}"#,
    );
    assert_eq!(resolved.status(), StatusCode::NOT_IMPLEMENTED);
}

#[test]
fn run_rejects_row_batches_that_do_not_match_mapping_source_contract() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );
    postgres.exec(
        "app_a",
        "CREATE TABLE public.orders (id bigint PRIMARY KEY, total_cents bigint NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config_with_tables(
        &config_path,
        bind_port,
        &["public.customers", "public.orders"],
    );

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let missing_source = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        r#"{"length":1,"payload":[{"after":{"id":1},"before":null,"key":{"id":1},"op":"c","ts_ns":1}]}"#,
    );
    assert_eq!(missing_source.status(), StatusCode::BAD_REQUEST);

    let wrong_database = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_b", "customers"),
    );
    assert_eq!(wrong_database.status(), StatusCode::BAD_REQUEST);

    let wrong_table = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "invoices"),
    );
    assert_eq!(wrong_table.status(), StatusCode::BAD_REQUEST);

    let mixed_tables = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &mixed_table_row_batch_body(),
    );
    assert_eq!(mixed_tables.status(), StatusCode::BAD_REQUEST);
}

fn row_batch_body(source_database: &str, table_name: &str) -> String {
    format!(
        r#"{{"length":1,"payload":[{{"after":{{"id":1,"email":"customer@example.com"}},"before":null,"key":{{"id":1}},"op":"c","source":{{"database_name":"{source_database}","schema_name":"public","table_name":"{table_name}"}},"ts_ns":1}}]}}"#
    )
}

fn mixed_table_row_batch_body() -> String {
    r#"{"length":2,"payload":[{"after":{"id":1,"email":"customer@example.com"},"before":null,"key":{"id":1},"op":"c","source":{"database_name":"demo_a","schema_name":"public","table_name":"customers"},"ts_ns":1},{"after":{"id":2,"total_cents":1500},"before":null,"key":{"id":2},"op":"c","source":{"database_name":"demo_a","schema_name":"public","table_name":"orders"},"ts_ns":2}]}"#.to_owned()
}
