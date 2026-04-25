use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use predicates::prelude::{PredicateBooleanExt, predicate};
use reqwest::{Certificate, blocking::Client};
use serde_json::Value;

#[path = "support/host_process_runner.rs"]
mod runner_process_support;
#[path = "support/host_process_runner_failure.rs"]
mod runner_process_support_failure;

use runner_process_support::HostProcessRunner;

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

            let mut instance = Self {
                _data_dir: data_dir,
                process,
                port,
            };
            if instance.wait_until_ready() {
                return instance;
            }
        }

        panic!("postgres test cluster could not claim a stable TCP port");
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

    fn wait_until_ready(&mut self) -> bool {
        for _ in 0..50 {
            if self
                .process
                .try_wait()
                .expect("postgres process status should be readable")
                .is_some()
            {
                return false;
            }

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
                return self.server_data_directory_matches_expected();
            }
            thread::sleep(Duration::from_millis(100));
        }

        false
    }

    fn server_data_directory_matches_expected(&self) -> bool {
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
                == self._data_dir.path().display().to_string()
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
}

impl Drop for TestPostgres {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
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
        "CREATE ROLE app_owner_a LOGIN PASSWORD 'owner-secret-a';",
    );
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER app_owner_a;");
    postgres.exec(
        "app_a",
        "GRANT CONNECT, CREATE ON DATABASE app_a TO migration_user_a;",
    );
    postgres.exec_as(
        "app_owner_a",
        "app_a",
        "CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
         GRANT USAGE ON SCHEMA public TO migration_user_a;
         GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE public.customers TO migration_user_a;",
    );

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, 0);
    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());

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
fn run_supports_json_operator_logs_for_runtime_startup() {
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

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, 0);
    let mut runner = HostProcessRunner::start(&config_path);

    let payload = runner.read_stderr_event();
    let json_object = payload
        .as_object()
        .expect("runner startup log should be a json object");

    for key in ["timestamp", "level", "service", "event", "message"] {
        assert!(
            json_object.contains_key(key),
            "runner startup json log must include `{key}`: {payload}",
        );
    }

    assert_eq!(
        json_object.get("service").and_then(Value::as_str),
        Some("runner"),
        "runner startup json log must identify the runner service",
    );
    assert_eq!(
        json_object.get("event").and_then(Value::as_str),
        Some("runtime.starting"),
        "runner startup json log must expose the runtime startup event",
    );
    runner.assert_healthy(&https_client());
}

#[test]
fn run_serves_healthz_after_binding_an_ephemeral_webhook_port() {
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

    let _stale_port_owner =
        TcpListener::bind("127.0.0.1:0").expect("stale candidate port should be occupiable");
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, 0);
    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());
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

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config_with_tables(
        &config_path,
        0,
        &["public.customers", "public.orders"],
    );
    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());

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

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config_with_tables(
        &config_path,
        0,
        &["public.customers", "public.orders"],
    );

    {
        let mut runner = HostProcessRunner::start(&config_path);
        runner.assert_healthy(&https_client());
    }

    postgres.exec(
        "app_a",
        "UPDATE _cockroach_migration_tool.table_sync_state
         SET last_successful_sync_watermark = '1776526353000000000.0000000000',
             last_error = 'kept on restart'
         WHERE mapping_id = 'app-a'
           AND source_table_name = 'public.customers';",
    );

    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());

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

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config_with_tables(
        &config_path,
        0,
        &["public.customers", "public.orders"],
    );

    {
        let mut runner = HostProcessRunner::start(&config_path);
        runner.assert_healthy(&https_client());
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

    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());

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

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_shared_destination_runner_config(
        &config_path,
        0,
        &["public.customers"],
        &["public.orders"],
    );

    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());

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
fn run_bootstraps_shared_destination_helper_state_when_mappings_mix_url_and_decomposed_targets() {
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

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    fs::write(
        &config_path,
        format!(
            r#"webhook:
  bind_addr: 127.0.0.1:0
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
        - public.customers
    destination:
      url: postgresql://migration_user_shared:runner-secret-shared@127.0.0.1:{port}/shared_app
  - id: app-b
    source:
      database: demo_b
      tables:
        - public.orders
    destination:
      host: 127.0.0.1
      port: {port}
      database: shared_app
      user: migration_user_shared
      password: runner-secret-shared
"#,
            cert_path = fixture_path("certs/server.crt").display(),
            key_path = fixture_path("certs/server.key").display(),
            port = postgres.port,
        ),
    )
    .expect("mixed destination runner config should be written");

    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());

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

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, 0);
    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());

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

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config_with_tables(&config_path, 0, &["public.orders"]);
    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());

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

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path, 0);
    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());

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
    postgres.write_runner_config_with_tables(&config_path, 0, &["public.missing_table"]);

    HostProcessRunner::start(&config_path).assert_exits_failure(predicate::str::contains(
        "postgres bootstrap: missing mapped destination table `public.missing_table` for mapping `app-a` in `app_a`",
    ));
}

#[test]
fn run_reports_startup_failures_as_json_error_events() {
    let postgres = TestPostgres::start();
    postgres.exec(
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    postgres.exec("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config_with_tables(&config_path, 0, &["public.missing_table"]);

    let mut runner = HostProcessRunner::start(&config_path);
    let (stdout, stderr) = runner.wait_for_failed_exit_logs();

    assert!(
        stdout.is_empty(),
        "json logging mode must keep startup failure stdout empty, got: {stdout:?}",
    );

    let lines: Vec<&str> = stderr.lines().collect();
    assert!(
        lines.len() >= 2,
        "runner json startup failure should log both startup and failure events, got: {stderr:?}",
    );

    let mut saw_startup = false;
    let mut saw_failure = false;
    for line in lines {
        let payload: Value =
            serde_json::from_str(line).expect("runner startup failure logs must stay valid json");
        let json_object = payload
            .as_object()
            .expect("runner startup failure log must be a json object");
        for key in ["timestamp", "level", "service", "event", "message"] {
            assert!(
                json_object.contains_key(key),
                "runner startup failure json log must include `{key}`: {payload}",
            );
        }
        assert_eq!(
            json_object.get("service").and_then(Value::as_str),
            Some("runner"),
            "runner startup failure json log must identify the runner service",
        );
        match json_object.get("event").and_then(Value::as_str) {
            Some("runtime.starting") => saw_startup = true,
            Some("command.failed") => {
                saw_failure = true;
                let message = json_object
                    .get("message")
                    .and_then(Value::as_str)
                    .expect("runner failure event must expose the failure message");
                assert!(
                    message.contains("missing mapped destination table"),
                    "runner failure event must retain the explicit startup failure detail, got: {message:?}",
                );
            }
            Some(other) => panic!("unexpected runner json startup event `{other}`"),
            None => panic!("runner startup failure log must include a string event field"),
        }
    }

    assert!(
        saw_startup,
        "runner json startup failure must log the startup event"
    );
    assert!(
        saw_failure,
        "runner json startup failure must log the failure event"
    );
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
        0,
        &["public.customers"],
        &["public.customers"],
    );

    HostProcessRunner::start(&config_path).assert_exits_failure(
        predicate::str::contains("destination database `127.0.0.1:").and(predicate::str::contains(
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
        0,
        &["public.customers"],
        ("migration_user_shared_a", "runner-secret-shared-a"),
        &["public.orders"],
        ("migration_user_shared_b", "runner-secret-shared-b"),
    );

    HostProcessRunner::start(&config_path).assert_exits_failure(
        predicate::str::contains("destination database `127.0.0.1:").and(predicate::str::contains(
            "has conflicting PostgreSQL target contracts for mappings `app-a` and `app-b`",
        )),
    );
}
