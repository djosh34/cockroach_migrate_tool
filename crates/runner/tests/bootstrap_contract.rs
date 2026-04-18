use std::{
    fs,
    io::Read,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use predicates::prelude::{PredicateBooleanExt, predicate};
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
        self.exec_as("postgres", database, sql);
    }

    fn exec_as(&self, user: &str, database: &str, sql: &str) {
        run_command(
            Command::new("psql")
                .env("PGPASSWORD", "")
                .args([
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

    fn write_shared_destination_runner_config(
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

    fn write_shared_destination_runner_config_with_credentials(
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
verify:
  molt:
    command: molt
    report_dir: /tmp/molt
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
{app_a_tables_yaml}
    destination:
      connection:
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
      connection:
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

    fn assert_exits_failure(
        mut self,
        stderr_predicate: impl predicates::Predicate<str>,
    ) {
        for _ in 0..50 {
            if let Some(status) = self
                .child
                .try_wait()
                .expect("runner child status should be readable")
            {
                assert!(
                    !status.success(),
                    "runner unexpectedly exited successfully with status {status}"
                );

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

                assert!(
                    stderr_predicate.eval(&stderr),
                    "runner stderr did not match expectation\nstdout:\n{stdout}\nstderr:\n{stderr}"
                );
                return;
            }

            thread::sleep(Duration::from_millis(100));
        }

        panic!("runner stayed up instead of failing during startup");
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
fn run_bootstraps_helper_schema_and_tracking_tables_in_destination_database() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec(
        "app_a",
        "GRANT ALL PRIVILEGES ON DATABASE app_a TO migration_user_a;",
    );
    postgres.exec_as(
        "migration_user_a",
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

    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT EXISTS (\n  SELECT 1\n  FROM information_schema.schemata\n  WHERE schema_name = '_cockroach_migration_tool'\n);",
        ),
        "t"
    );
    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT string_agg(table_name, ',' ORDER BY table_name)\nFROM information_schema.tables\nWHERE table_schema = '_cockroach_migration_tool';",
        ),
        "app-a__public__customers,stream_state,table_sync_state"
    );
}

#[test]
fn run_seeds_tracking_rows_for_stream_and_each_mapped_table() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );
    postgres.exec_as(
        "migration_user_a",
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
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(
        &format!("https://localhost:{bind_port}/healthz"),
        &https_client(),
    );

    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT mapping_id || ':' || source_database || ':' || COALESCE(stream_status, '<null>')\n\
             FROM _cockroach_migration_tool.stream_state;",
        ),
        "app-a:demo_a:bootstrap_pending"
    );
    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT string_agg(\n\
                mapping_id || ':' || source_table_name || ':' || helper_table_name,\n\
                ',' ORDER BY source_table_name\n\
             )\n\
             FROM _cockroach_migration_tool.table_sync_state;",
        ),
        "app-a:public.customers:app-a__public__customers,app-a:public.orders:app-a__public__orders"
    );
}

#[test]
fn run_preserves_existing_table_sync_progress_on_restart() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );
    postgres.exec_as(
        "migration_user_a",
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

    {
        let mut runner = RunnerProcess::start(&config_path);
        runner.assert_healthy(
            &format!("https://localhost:{bind_port}/healthz"),
            &https_client(),
        );
    }

    postgres.exec(
        "app_a",
        "UPDATE _cockroach_migration_tool.table_sync_state
         SET last_successful_sync_watermark = '1776526353000000000.0000000000',
             last_error = 'kept on restart'
         WHERE mapping_id = 'app-a'
           AND source_table_name = 'public.customers';",
    );

    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(
        &format!("https://localhost:{bind_port}/healthz"),
        &https_client(),
    );

    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT string_agg(
                source_table_name || ':' || helper_table_name || ':' ||
                COALESCE(last_successful_sync_watermark, '<null>') || ':' ||
                COALESCE(last_error, '<null>'),
                ',' ORDER BY source_table_name
             )
             FROM _cockroach_migration_tool.table_sync_state
             WHERE mapping_id = 'app-a';",
        ),
        "public.customers:app-a__public__customers:1776526353000000000.0000000000:kept on restart,public.orders:app-a__public__orders:<null>:<null>"
    );
}

#[test]
fn run_preserves_existing_stream_and_table_tracking_progress_on_restart() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );
    postgres.exec_as(
        "migration_user_a",
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
             last_error = 'kept on restart'
         WHERE mapping_id = 'app-a'
           AND source_table_name = 'public.customers';",
    );

    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(
        &format!("https://localhost:{bind_port}/healthz"),
        &https_client(),
    );

    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT COALESCE(latest_received_resolved_watermark, '<null>') || ':' || \
                    COALESCE(latest_reconciled_resolved_watermark, '<null>')
             FROM _cockroach_migration_tool.stream_state
             WHERE mapping_id = 'app-a';",
        ),
        "1776526353000000000.0000000001:1776526353000000000.0000000000"
    );
    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT string_agg(
                source_table_name || ':' || helper_table_name || ':' ||
                COALESCE(last_successful_sync_watermark, '<null>') || ':' ||
                COALESCE(last_error, '<null>'),
                ',' ORDER BY source_table_name
             )
             FROM _cockroach_migration_tool.table_sync_state
             WHERE mapping_id = 'app-a';",
        ),
        "public.customers:app-a__public__customers:1776526353000000000.0000000000:kept on restart,public.orders:app-a__public__orders:<null>:<null>"
    );
}

#[test]
fn run_bootstraps_shared_destination_helper_state_per_mapping() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_shared LOGIN PASSWORD 'runner-secret-shared';",
    );
    postgres.exec(
        "postgres",
        "CREATE DATABASE shared_app OWNER migration_user_shared;",
    );
    postgres.exec_as(
        "migration_user_shared",
        "shared_app",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
         CREATE TABLE public.orders (id bigint PRIMARY KEY, total_cents bigint NOT NULL);",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_shared_destination_runner_config(
        &config_path,
        bind_port,
        &["public.customers"],
        &["public.orders"],
    );

    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(
        &format!("https://localhost:{bind_port}/healthz"),
        &https_client(),
    );

    assert_eq!(
        postgres.query(
            "shared_app",
            "SELECT string_agg(table_name, ',' ORDER BY table_name)
             FROM information_schema.tables
             WHERE table_schema = '_cockroach_migration_tool';",
        ),
        "app-a__public__customers,app-b__public__orders,stream_state,table_sync_state"
    );
    assert_eq!(
        postgres.query(
            "shared_app",
            "SELECT string_agg(mapping_id || ':' || source_database, ',' ORDER BY mapping_id)
             FROM _cockroach_migration_tool.stream_state;",
        ),
        "app-a:demo_a,app-b:demo_b"
    );
    assert_eq!(
        postgres.query(
            "shared_app",
            "SELECT string_agg(
                mapping_id || ':' || source_table_name || ':' || helper_table_name,
                ',' ORDER BY mapping_id
             )
             FROM _cockroach_migration_tool.table_sync_state;",
        ),
        "app-a:public.customers:app-a__public__customers,app-b:public.orders:app-b__public__orders"
    );
}

#[test]
fn run_prepares_a_helper_shadow_table_for_each_mapped_destination_table() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.customers (
            id bigint PRIMARY KEY,
            email text NOT NULL,
            nickname text DEFAULT 'friend'
        );",
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

    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT string_agg(column_name || ':' || data_type, ',' ORDER BY ordinal_position)
             FROM information_schema.columns
             WHERE table_schema = '_cockroach_migration_tool'
               AND table_name = 'app-a__public__customers';",
        ),
        "id:bigint,email:text,nickname:text"
    );
}

#[test]
fn run_adds_one_automatic_helper_index_for_primary_key_columns() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.orders (
            tenant_id bigint NOT NULL,
            order_id bigint NOT NULL,
            total_cents bigint NOT NULL,
            PRIMARY KEY (tenant_id, order_id)
        );",
    );

    let bind_port = pick_unused_port();
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config_with_tables(&config_path, bind_port, &["public.orders"]);
    let mut runner = RunnerProcess::start(&config_path);
    runner.assert_healthy(
        &format!("https://localhost:{bind_port}/healthz"),
        &https_client(),
    );

    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT string_agg(indexdef, E'\\n' ORDER BY indexname)
             FROM pg_indexes
             WHERE schemaname = '_cockroach_migration_tool'
               AND tablename = 'app-a__public__orders';",
        ),
        "CREATE UNIQUE INDEX \"app-a__public__orders__pk\" ON _cockroach_migration_tool.\"app-a__public__orders\" USING btree (tenant_id, order_id)"
    );
}

#[test]
fn run_helper_shadow_tables_drop_defaults_and_generated_expressions() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
    postgres.exec_as(
        "migration_user_a",
        "app_a",
        "CREATE TABLE public.customers (
            id bigint PRIMARY KEY,
            email text NOT NULL DEFAULT 'friend@example.com',
            email_length bigint GENERATED ALWAYS AS (char_length(email)) STORED
        );",
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

    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT string_agg(
                column_name || ':' || is_nullable || ':' || COALESCE(column_default, '<none>') || ':' || is_generated,
                ',' ORDER BY ordinal_position
             )
             FROM information_schema.columns
             WHERE table_schema = '_cockroach_migration_tool'
               AND table_name = 'app-a__public__customers';",
        ),
        "id:NO:<none>:NEVER,email:NO:<none>:NEVER,email_length:YES:<none>:NEVER"
    );
}

#[test]
fn run_fails_loudly_when_a_mapped_destination_table_is_missing() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config_with_tables(
        &config_path,
        pick_unused_port(),
        &["public.missing_table"],
    );

    RunnerProcess::start(&config_path).assert_exits_failure(predicate::str::contains(
        "postgres bootstrap: missing mapped destination table `public.missing_table` for mapping `app-a` in `app_a`",
    ));
}

#[test]
fn run_fails_loudly_when_two_mappings_share_one_destination_table() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_shared LOGIN PASSWORD 'runner-secret-shared';",
    );
    postgres.exec(
        "postgres",
        "CREATE DATABASE shared_app OWNER migration_user_shared;",
    );
    postgres.exec_as(
        "migration_user_shared",
        "shared_app",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);",
    );

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_shared_destination_runner_config(
        &config_path,
        pick_unused_port(),
        &["public.customers"],
        &["public.customers"],
    );

    RunnerProcess::start(&config_path).assert_exits_failure(
        predicate::str::contains("destination database `127.0.0.1:")
            .and(predicate::str::contains(
                "`public.customers` is claimed by both mappings `app-a` and `app-b`",
            )),
    );
}

#[test]
fn run_fails_loudly_when_two_mappings_share_one_destination_with_conflicting_credentials() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_shared_a LOGIN PASSWORD 'runner-secret-shared-a';",
    );
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_shared_b LOGIN PASSWORD 'runner-secret-shared-b';",
    );
    postgres.exec(
        "postgres",
        "CREATE DATABASE shared_app OWNER migration_user_shared_a;",
    );
    postgres.exec(
        "shared_app",
        "GRANT ALL PRIVILEGES ON DATABASE shared_app TO migration_user_shared_b;",
    );
    postgres.exec_as(
        "migration_user_shared_a",
        "shared_app",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
         CREATE TABLE public.orders (id bigint PRIMARY KEY, total_cents bigint NOT NULL);",
    );
    postgres.exec(
        "shared_app",
        "GRANT USAGE ON SCHEMA public TO migration_user_shared_b;
         GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO migration_user_shared_b;",
    );

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_shared_destination_runner_config_with_credentials(
        &config_path,
        pick_unused_port(),
        &["public.customers"],
        ("migration_user_shared_a", "runner-secret-shared-a"),
        &["public.orders"],
        ("migration_user_shared_b", "runner-secret-shared-b"),
    );

    RunnerProcess::start(&config_path).assert_exits_failure(
        predicate::str::contains("destination database `127.0.0.1:")
            .and(predicate::str::contains(
                "has conflicting connection contracts for mappings `app-a` and `app-b`",
            )),
    );
}
