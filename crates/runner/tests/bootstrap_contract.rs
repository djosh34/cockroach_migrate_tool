use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
};

#[path = "support/predicates.rs"]
mod predicates;

use predicates::prelude::{PredicateBooleanExt, predicate};
use reqwest::{Certificate, Identity, blocking::Client};
use serde_json::Value;

#[path = "support/host_process_runner.rs"]
mod runner_process_support;
#[path = "support/host_process_runner_failure.rs"]
mod runner_process_support_failure;
#[path = "support/test_postgres.rs"]
mod test_postgres_support;

use runner_process_support::HostProcessRunner;
use test_postgres_support::TestPostgres;

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

fn https_client_with_ca(certificate_path: &Path) -> Client {
    let certificate = Certificate::from_pem(
        &fs::read(certificate_path).expect("server certificate should be readable"),
    )
    .expect("server certificate should parse");

    Client::builder()
        .add_root_certificate(certificate)
        .build()
        .expect("https client should build")
}

fn https_client_with_identity(certificate_path: &Path, identity: Identity) -> Client {
    let certificate = Certificate::from_pem(
        &fs::read(certificate_path).expect("server certificate should be readable"),
    )
    .expect("server certificate should parse");

    Client::builder()
        .add_root_certificate(certificate)
        .identity(identity)
        .build()
        .expect("https client with identity should build")
}

fn generate_mtls_client_identity(temp_dir: &Path) -> Identity {
    let client_cert_config_path = temp_dir.join("runner-client.cnf");
    fs::write(
        &client_cert_config_path,
        "[req]\n\
         distinguished_name = dn\n\
         prompt = no\n\
         req_extensions = req_ext\n\
         \n\
         [dn]\n\
         CN = runner-test-client\n\
         \n\
         [req_ext]\n\
         basicConstraints = CA:FALSE\n\
         keyUsage = critical,digitalSignature,keyEncipherment\n\
         extendedKeyUsage = clientAuth\n",
    )
    .expect("runner mtls client config should be written");

    let client_key_path = temp_dir.join("runner-client.key");
    let client_csr_path = temp_dir.join("runner-client.csr");
    let client_cert_path = temp_dir.join("runner-client.crt");
    let client_serial_path = temp_dir.join("runner-client.srl");

    run_command(
        Command::new("openssl").args([
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            client_key_path
                .to_str()
                .expect("runner client key path should be utf-8"),
            "-out",
            client_csr_path
                .to_str()
                .expect("runner client csr path should be utf-8"),
            "-config",
            client_cert_config_path
                .to_str()
                .expect("runner client config path should be utf-8"),
        ]),
        "openssl req runner mtls client csr",
    );
    run_command(
        Command::new("openssl").args([
            "x509",
            "-req",
            "-days",
            "365",
            "-in",
            client_csr_path
                .to_str()
                .expect("runner client csr path should be utf-8"),
            "-CA",
            investigation_cert("ca.crt")
                .to_str()
                .expect("runner investigation ca cert path should be utf-8"),
            "-CAkey",
            investigation_cert("ca.key")
                .to_str()
                .expect("runner investigation ca key path should be utf-8"),
            "-CAcreateserial",
            "-CAserial",
            client_serial_path
                .to_str()
                .expect("runner client serial path should be utf-8"),
            "-out",
            client_cert_path
                .to_str()
                .expect("runner client cert path should be utf-8"),
            "-extensions",
            "req_ext",
            "-extfile",
            client_cert_config_path
                .to_str()
                .expect("runner client config path should be utf-8"),
        ]),
        "openssl x509 runner mtls client cert",
    );

    let mut pem =
        fs::read(&client_cert_path).expect("runner mtls client certificate should be readable");
    pem.extend_from_slice(
        &fs::read(&client_key_path).expect("runner mtls client key should be readable"),
    );
    Identity::from_pem(&pem).expect("runner mtls client identity should parse")
}

fn http_client() -> Client {
    Client::builder().build().expect("http client should build")
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
fn run_serves_plain_http_healthz_when_webhook_mode_is_http() {
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
    postgres.write_http_runner_config(&config_path, 0);
    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&http_client());
}

#[test]
fn run_serves_https_healthz_when_webhook_mode_is_explicit_https() {
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
    postgres.write_explicit_https_runner_config(&config_path, 0);
    let mut runner = HostProcessRunner::start(&config_path);
    runner.assert_healthy(&https_client());
}

#[test]
fn run_requires_a_client_certificate_when_webhook_client_ca_is_configured() {
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
    postgres.write_mtls_runner_config(&config_path, 0);

    let mut runner = HostProcessRunner::start(&config_path);
    let healthz_url = runner.healthz_url();
    let unauthenticated_client = https_client_with_ca(&investigation_cert("ca.crt"));
    let authenticated_client = https_client_with_identity(
        &investigation_cert("ca.crt"),
        generate_mtls_client_identity(temp_dir.path()),
    );

    assert!(
        unauthenticated_client.get(&healthz_url).send().is_err(),
        "runner mtls listener must reject clients without a certificate",
    );
    runner.assert_healthy(&authenticated_client);
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
            port = postgres.port(),
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
