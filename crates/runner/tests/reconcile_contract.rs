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
                "-c",
                sql,
            ])
            .output()
            .expect("psql should start");
        assert!(
            output.status.success(),
            "psql failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn query(&self, database: &str, sql: &str) -> String {
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
                "-At",
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

    fn write_runner_config(
        &self,
        path: &Path,
        bind_port: u16,
        reconcile_interval_secs: u64,
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
  interval_secs: {reconcile_interval_secs}
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
            ),
        )
        .expect("runner config should be written");
    }

    fn write_multi_mapping_runner_config(
        &self,
        path: &Path,
        bind_port: u16,
        reconcile_interval_secs: u64,
    ) {
        fs::write(
            path,
            format!(
                r#"webhook:
  bind_addr: 127.0.0.1:{bind_port}
  tls:
    cert_path: {cert_path}
    key_path: {key_path}
reconcile:
  interval_secs: {reconcile_interval_secs}
verify:
  molt:
    command: molt
    report_dir: /tmp/molt
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
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.invoices
    destination:
      connection:
        host: 127.0.0.1
        port: {postgres_port}
        database: app_b
        user: migration_user_b
        password: runner-secret-b
"#,
                cert_path = fixture_path("certs/server.crt").display(),
                key_path = fixture_path("certs/server.key").display(),
                postgres_port = self.port,
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
                Ok(response) if response.status().is_success() => return,
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

fn row_batch_body(source_database: &str, table_name: &str, email: &str) -> String {
    format!(
        r#"{{"length":1,"payload":[{{"after":{{"id":1,"email":"{email}"}},"before":null,"key":{{"id":1}},"op":"c","source":{{"database_name":"{source_database}","schema_name":"public","table_name":"{table_name}"}},"ts_ns":1}}]}}"#
    )
}

fn delete_row_batch_body(source_database: &str, table_name: &str) -> String {
    format!(
        r#"{{"length":1,"payload":[{{"after":null,"before":{{"id":1,"email":"customer@example.com"}},"key":{{"id":1}},"op":"d","source":{{"database_name":"{source_database}","schema_name":"public","table_name":"{table_name}"}},"ts_ns":2}}]}}"#
    )
}

fn order_delete_row_batch_body(source_database: &str) -> String {
    format!(
        r#"{{"length":1,"payload":[{{"after":null,"before":{{"id":10,"customer_id":1,"total_cents":1500}},"key":{{"id":10}},"op":"d","source":{{"database_name":"{source_database}","schema_name":"public","table_name":"orders"}},"ts_ns":3}}]}}"#
    )
}

fn order_row_batch_body(source_database: &str, customer_id: i64, total_cents: i64) -> String {
    format!(
        r#"{{"length":1,"payload":[{{"after":{{"id":10,"customer_id":{customer_id},"total_cents":{total_cents}}},"before":null,"key":{{"id":10}},"op":"c","source":{{"database_name":"{source_database}","schema_name":"public","table_name":"orders"}},"ts_ns":2}}]}}"#
    )
}

fn invoice_row_batch_body(source_database: &str, amount_cents: i64) -> String {
    format!(
        r#"{{"length":1,"payload":[{{"after":{{"id":22,"amount_cents":{amount_cents}}},"before":null,"key":{{"id":22}},"op":"c","source":{{"database_name":"{source_database}","schema_name":"public","table_name":"invoices"}},"ts_ns":3}}]}}"#
    )
}

fn invoice_delete_row_batch_body(source_database: &str) -> String {
    format!(
        r#"{{"length":1,"payload":[{{"after":null,"before":{{"id":22,"amount_cents":4200}},"key":{{"id":22}},"op":"d","source":{{"database_name":"{source_database}","schema_name":"public","table_name":"invoices"}},"ts_ns":4}}]}}"#
    )
}

fn resolved_body(watermark: &str) -> String {
    format!(r#"{{"resolved":"{watermark}"}}"#)
}

fn assert_eventually_query_equals(
    postgres: &TestPostgres,
    database: &str,
    sql: &str,
    expected: &str,
) {
    for _ in 0..40 {
        if postgres.query(database, sql) == expected {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }

    assert_eq!(postgres.query(database, sql), expected);
}

#[test]
fn run_continuously_reconciles_helper_upserts_into_real_tables() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "SET ROLE migration_user_a;
         CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, bind_port, 1, &["public.customers"]);

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers", "customer@example.com"),
    );
    assert_eq!(response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT COALESCE(email, '<null>') FROM public.customers WHERE id = 1;",
        "customer@example.com",
    );
}

#[test]
fn run_continuously_reconciles_helper_deletes_into_real_tables() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "SET ROLE migration_user_a;
         CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, bind_port, 1, &["public.customers"]);

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let upsert_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers", "customer@example.com"),
    );
    assert_eq!(upsert_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT COALESCE(email, '<null>') FROM public.customers WHERE id = 1;",
        "customer@example.com",
    );

    let delete_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &delete_row_batch_body("demo_a", "customers"),
    );
    assert_eq!(delete_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT count(*)::text FROM public.customers WHERE id = 1;",
        "0",
    );
}

#[test]
fn run_reconciles_tables_in_dependency_order_not_config_order() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "SET ROLE migration_user_a;
         CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
         CREATE TABLE public.orders (
             id bigint PRIMARY KEY,
             customer_id bigint NOT NULL REFERENCES public.customers (id),
             total_cents bigint NOT NULL
         );",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(
        &config_path,
        bind_port,
        1,
        &["public.orders", "public.customers"],
    );

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let order_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &order_row_batch_body("demo_a", 1, 1500),
    );
    assert_eq!(order_response.status(), StatusCode::OK);

    let customer_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers", "fk@example.com"),
    );
    assert_eq!(customer_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT customer_id || ':' || total_cents FROM public.orders WHERE id = 10;",
        "1:1500",
    );
}

#[test]
fn run_reconciles_deletes_in_reverse_dependency_order() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "SET ROLE migration_user_a;
         CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
         CREATE TABLE public.orders (
             id bigint PRIMARY KEY,
             customer_id bigint NOT NULL REFERENCES public.customers (id),
             total_cents bigint NOT NULL
         );",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(
        &config_path,
        bind_port,
        1,
        &["public.orders", "public.customers"],
    );

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let customer_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers", "fk-delete@example.com"),
    );
    assert_eq!(customer_response.status(), StatusCode::OK);

    let order_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &order_row_batch_body("demo_a", 1, 1500),
    );
    assert_eq!(order_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT count(*)::text FROM public.orders WHERE id = 10;",
        "1",
    );

    let customer_delete_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &delete_row_batch_body("demo_a", "customers"),
    );
    assert_eq!(customer_delete_response.status(), StatusCode::OK);

    let order_delete_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &order_delete_row_batch_body("demo_a"),
    );
    assert_eq!(order_delete_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT count(*)::text FROM public.orders;",
        "0",
    );
    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT count(*)::text FROM public.customers;",
        "0",
    );
}

#[test]
fn run_repeats_upsert_reconcile_without_duplication() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "SET ROLE migration_user_a;
         CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, bind_port, 1, &["public.customers"]);

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers", "stable@example.com"),
    );
    assert_eq!(response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT COALESCE(email, '<null>') FROM public.customers WHERE id = 1;",
        "stable@example.com",
    );

    thread::sleep(Duration::from_millis(2200));

    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT count(*) || ':' || max(email) FROM public.customers WHERE id = 1;",
        ),
        "1:stable@example.com"
    );
}

#[test]
fn run_repeats_delete_reconcile_without_errors_or_reinserts() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "SET ROLE migration_user_a;
         CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, bind_port, 1, &["public.customers"]);

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let upsert_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers", "gone@example.com"),
    );
    assert_eq!(upsert_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT count(*)::text FROM public.customers WHERE id = 1;",
        "1",
    );

    let delete_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &delete_row_batch_body("demo_a", "customers"),
    );
    assert_eq!(delete_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT count(*)::text FROM public.customers WHERE id = 1;",
        "0",
    );

    thread::sleep(Duration::from_millis(2200));

    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT count(*)::text FROM public.customers WHERE id = 1;",
        ),
        "0"
    );
}

#[test]
fn run_reconciles_each_mapping_into_only_its_own_destination_database() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_b LOGIN PASSWORD 'runner-secret-b';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec("postgres", "CREATE DATABASE app_b OWNER migration_user_b;");
    postgres.exec(
        "app_a",
        "SET ROLE migration_user_a;
         CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );
    postgres.exec(
        "app_b",
        "SET ROLE migration_user_b;
         CREATE TABLE public.invoices (id bigint PRIMARY KEY, amount_cents bigint NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_multi_mapping_runner_config(&config_path, bind_port, 1);

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let customer_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers", "isolated@example.com"),
    );
    assert_eq!(customer_response.status(), StatusCode::OK);

    let invoice_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-b"),
        &client,
        &invoice_row_batch_body("demo_b", 4200),
    );
    assert_eq!(invoice_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT COALESCE(email, '<null>') FROM public.customers WHERE id = 1;",
        "isolated@example.com",
    );
    assert_eventually_query_equals(
        &postgres,
        "app_b",
        "SELECT COALESCE(amount_cents::text, '<null>') FROM public.invoices WHERE id = 22;",
        "4200",
    );
}

#[test]
fn run_reconciles_deletes_only_within_the_target_mapping_database() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_b LOGIN PASSWORD 'runner-secret-b';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec("postgres", "CREATE DATABASE app_b OWNER migration_user_b;");
    postgres.exec(
        "app_a",
        "SET ROLE migration_user_a;
         CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );
    postgres.exec(
        "app_b",
        "SET ROLE migration_user_b;
         CREATE TABLE public.invoices (id bigint PRIMARY KEY, amount_cents bigint NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_multi_mapping_runner_config(&config_path, bind_port, 1);

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let customer_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers", "isolated-delete@example.com"),
    );
    assert_eq!(customer_response.status(), StatusCode::OK);

    let invoice_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-b"),
        &client,
        &invoice_row_batch_body("demo_b", 4200),
    );
    assert_eq!(invoice_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT count(*)::text FROM public.customers WHERE id = 1;",
        "1",
    );
    assert_eventually_query_equals(
        &postgres,
        "app_b",
        "SELECT count(*)::text FROM public.invoices WHERE id = 22;",
        "1",
    );

    let customer_delete_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &delete_row_batch_body("demo_a", "customers"),
    );
    assert_eq!(customer_delete_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT count(*)::text FROM public.customers WHERE id = 1;",
        "0",
    );
    assert_eventually_query_equals(
        &postgres,
        "app_b",
        "SELECT COALESCE(amount_cents::text, '<null>') FROM public.invoices WHERE id = 22;",
        "4200",
    );

    let invoice_delete_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-b"),
        &client,
        &invoice_delete_row_batch_body("demo_b"),
    );
    assert_eq!(invoice_delete_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_b",
        "SELECT count(*)::text FROM public.invoices WHERE id = 22;",
        "0",
    );
}

#[test]
fn run_advances_success_tracking_after_a_full_upsert_pass() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "SET ROLE migration_user_a;
         CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, bind_port, 1, &["public.customers"]);

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let row_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers", "tracked@example.com"),
    );
    assert_eq!(row_response.status(), StatusCode::OK);

    let watermark = "1776526353000000000.0000000000";
    let resolved_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &resolved_body(watermark),
    );
    assert_eq!(resolved_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT COALESCE(email, '<null>') FROM public.customers WHERE id = 1;",
        "tracked@example.com",
    );
    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT COALESCE(latest_reconciled_resolved_watermark, '<null>') \
         FROM _cockroach_migration_tool.stream_state \
         WHERE mapping_id = 'app-a';",
        watermark,
    );
    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT (last_successful_sync_time IS NOT NULL)::text || ':' || \
                COALESCE(last_successful_sync_watermark, '<null>') \
         FROM _cockroach_migration_tool.table_sync_state \
         WHERE mapping_id = 'app-a' AND source_table_name = 'public.customers';",
        &format!("true:{watermark}"),
    );
}

#[test]
fn run_advances_success_tracking_after_a_full_delete_pass() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "SET ROLE migration_user_a;
         CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, bind_port, 1, &["public.customers"]);

    let client = https_client();
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(&format!("https://localhost:{bind_port}/healthz"), &client);

    let row_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &row_batch_body("demo_a", "customers", "tracked-delete@example.com"),
    );
    assert_eq!(row_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT count(*)::text FROM public.customers WHERE id = 1;",
        "1",
    );

    let delete_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &delete_row_batch_body("demo_a", "customers"),
    );
    assert_eq!(delete_response.status(), StatusCode::OK);

    let watermark = "1776526353000000000.0000000001";
    let resolved_response = runner.post(
        &format!("https://localhost:{bind_port}/ingest/app-a"),
        &client,
        &resolved_body(watermark),
    );
    assert_eq!(resolved_response.status(), StatusCode::OK);

    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT count(*)::text FROM public.customers WHERE id = 1;",
        "0",
    );
    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT COALESCE(latest_reconciled_resolved_watermark, '<null>') \
         FROM _cockroach_migration_tool.stream_state \
         WHERE mapping_id = 'app-a';",
        watermark,
    );
    assert_eventually_query_equals(
        &postgres,
        "app_a",
        "SELECT (last_successful_sync_time IS NOT NULL)::text || ':' || \
                COALESCE(last_successful_sync_watermark, '<null>') \
         FROM _cockroach_migration_tool.table_sync_state \
         WHERE mapping_id = 'app-a' AND source_table_name = 'public.customers';",
        &format!("true:{watermark}"),
    );
}
