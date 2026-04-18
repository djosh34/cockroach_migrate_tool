use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
    time::Duration,
};

use assert_cmd::Command as AssertCommand;
use predicates::prelude::predicate;

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

    fn write_runner_config(&self, path: &Path) {
        self.write_runner_config_with_tables(path, &["public.customers"]);
    }

    fn write_runner_config_with_tables(&self, path: &Path, tables: &[&str]) {
        let tables_yaml = tables
            .iter()
            .map(|table| format!("        - {table}"))
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(
            path,
            format!(
                r#"webhook:
  bind_addr: 127.0.0.1:8443
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
                cert_path = fixture_path("certs/server.crt").display(),
                key_path = fixture_path("certs/server.key").display(),
                tables_yaml = tables_yaml,
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

    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let config_path = temp_dir.path().join("runner.yml");
    postgres.write_runner_config(&config_path);

    let mut command = AssertCommand::cargo_bin("runner").expect("runner binary should exist");
    command
        .args(["run", "--config"])
        .arg(&config_path)
        .assert()
        .success();

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
    postgres.write_runner_config(&config_path);

    let mut command = AssertCommand::cargo_bin("runner").expect("runner binary should exist");
    command
        .args(["run", "--config"])
        .arg(&config_path)
        .assert()
        .success();

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
    postgres.write_runner_config_with_tables(&config_path, &["public.orders"]);

    let mut command = AssertCommand::cargo_bin("runner").expect("runner binary should exist");
    command
        .args(["run", "--config"])
        .arg(&config_path)
        .assert()
        .success();

    assert_eq!(
        postgres.query(
            "app_a",
            "SELECT string_agg(indexdef, E'\\n' ORDER BY indexname)
             FROM pg_indexes
             WHERE schemaname = '_cockroach_migration_tool'
               AND tablename = 'app-a__public__orders';",
        ),
        "CREATE INDEX \"app-a__public__orders__pk\" ON _cockroach_migration_tool.\"app-a__public__orders\" USING btree (tenant_id, order_id)"
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
    postgres.write_runner_config(&config_path);

    let mut command = AssertCommand::cargo_bin("runner").expect("runner binary should exist");
    command
        .args(["run", "--config"])
        .arg(&config_path)
        .assert()
        .success();

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
    postgres.write_runner_config_with_tables(&config_path, &["public.missing_table"]);

    let mut command = AssertCommand::cargo_bin("runner").expect("runner binary should exist");
    command
        .args(["run", "--config"])
        .arg(&config_path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "postgres bootstrap: missing mapped destination table `public.missing_table` for mapping `app-a` in `app_a`",
        ));
}
