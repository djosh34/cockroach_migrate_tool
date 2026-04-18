use std::{
    fs,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::{Certificate, blocking::Client};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .canonicalize()
        .expect("fixtures dir should resolve")
}

fn unique_suffix() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos()
        .to_string()
}

fn run_command(command: &mut Command, context: &str) -> String {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{context} should start: {error}"));
    assert!(
        output.status.success(),
        "{context} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("command output should be utf-8")
}

struct DockerHarness {
    image_tag: String,
    network_name: String,
    postgres_container: String,
    runner_container: String,
}

impl DockerHarness {
    fn new() -> Self {
        let suffix = unique_suffix();
        Self {
            image_tag: format!("cockroach-migrate-runner-test-{suffix}"),
            network_name: format!("cockroach-migrate-runner-net-{suffix}"),
            postgres_container: format!("cockroach-migrate-postgres-{suffix}"),
            runner_container: format!("cockroach-migrate-runner-{suffix}"),
        }
    }

    fn build_runner_image(&self) {
        let repo_root = repo_root();
        run_command(
            Command::new("docker")
                .args(["build", "-t", &self.image_tag])
                .arg(repo_root),
            "docker build",
        );
    }

    fn create_network(&self) {
        run_command(
            Command::new("docker").args(["network", "create", &self.network_name]),
            "docker network create",
        );
    }

    fn start_postgres(&self) {
        run_command(
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
                "-e",
                "POSTGRES_USER=postgres",
                "-e",
                "POSTGRES_PASSWORD=postgres",
                "-e",
                "POSTGRES_DB=postgres",
                "postgres:16",
            ]),
            "docker run postgres",
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

    fn exec_psql(&self, database: &str, sql: &str) -> String {
        run_command(
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

    fn prepare_postgres_schema(&self) {
        self.exec_psql(
            "postgres",
            "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';
             CREATE ROLE migration_user_b LOGIN PASSWORD 'runner-secret-b';",
        );
        self.exec_psql(
            "postgres",
            "CREATE DATABASE app_a OWNER migration_user_a;",
        );
        self.exec_psql(
            "postgres",
            "CREATE DATABASE app_b OWNER migration_user_b;",
        );
        self.exec_psql(
            "app_a",
            "SET ROLE migration_user_a;
             CREATE TABLE public.customers (id bigint PRIMARY KEY, email text NOT NULL);
             CREATE TABLE public.orders (
                 tenant_id bigint NOT NULL,
                 order_id bigint NOT NULL,
                 total_cents bigint NOT NULL,
                 PRIMARY KEY (tenant_id, order_id)
             );",
        );
        self.exec_psql(
            "app_b",
            "SET ROLE migration_user_b;
             CREATE TABLE public.invoices (id bigint PRIMARY KEY, amount_cents bigint NOT NULL);",
        );
    }

    fn start_runner(&self, fixture_mount: &str) {
        run_command(
            Command::new("docker").args([
                "run",
                "-d",
                "--rm",
                "--name",
                &self.runner_container,
                "--network",
                &self.network_name,
                "-p",
                "127.0.0.1::8443",
                "-v",
                fixture_mount,
                &self.image_tag,
                "run",
                "--config",
                "/config/container-runner-config.yml",
            ]),
            "docker run runner",
        );
    }

    fn runner_host_port(&self) -> u16 {
        let output = run_command(
            Command::new("docker").args([
                "port",
                &self.runner_container,
                "8443/tcp",
            ]),
            "docker port",
        );
        output
            .trim()
            .rsplit_once(':')
            .and_then(|(_, port)| port.parse::<u16>().ok())
            .expect("runner host port should parse")
    }
}

impl Drop for DockerHarness {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.runner_container])
            .output();
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.postgres_container])
            .output();
        let _ = Command::new("docker")
            .args(["network", "rm", &self.network_name])
            .output();
        let _ = Command::new("docker")
            .args(["image", "rm", "-f", &self.image_tag])
            .output();
    }
}

fn https_client() -> Client {
    let certificate = Certificate::from_pem(
        &fs::read(fixtures_dir().join("certs").join("server.crt"))
            .expect("server certificate should be readable"),
    )
    .expect("server certificate should parse");

    Client::builder()
        .add_root_certificate(certificate)
        .build()
        .expect("https client should build")
}

fn wait_for_runner_health(client: &Client, port: u16) {
    for _ in 0..60 {
        match client
            .get(format!("https://localhost:{port}/healthz"))
            .send()
        {
            Ok(response) if response.status().is_success() => return,
            Ok(_) | Err(_) => thread::sleep(Duration::from_secs(1)),
        }
    }

    panic!("runner container did not serve https://localhost:{port}/healthz");
}

#[test]
#[ignore = "long lane"]
fn ignored_long_lane_builds_and_runs_the_single_binary_runner_image_against_real_postgres() {
    let harness = DockerHarness::new();
    let fixtures_dir = fixtures_dir();
    let fixture_mount = format!(
        "{}:/config:ro",
        fixtures_dir
            .to_str()
            .expect("fixtures dir should be valid utf-8")
    );

    harness.build_runner_image();
    harness.create_network();
    harness.start_postgres();
    harness.wait_for_postgres();
    harness.prepare_postgres_schema();

    let inspect_stdout = run_command(
        Command::new("docker").args([
            "image",
            "inspect",
            &harness.image_tag,
            "--format",
            "{{json .Config.Entrypoint}}",
        ]),
        "docker image inspect",
    );
    assert_eq!(inspect_stdout.trim(), "[\"/usr/local/bin/runner\"]");

    let validate_stdout = run_command(
        Command::new("docker").args([
            "run",
            "--rm",
            "--network",
            &harness.network_name,
            "-v",
            &fixture_mount,
            &harness.image_tag,
            "validate-config",
            "--config",
            "/config/container-runner-config.yml",
        ]),
        "docker validate-config",
    );
    assert!(validate_stdout.contains("config=/config/container-runner-config.yml"));
    assert!(validate_stdout.contains("mappings=2"));
    assert!(validate_stdout.contains("verify=molt@/work/molt"));
    assert!(validate_stdout.contains("tls=/config/certs/server.crt+/config/certs/server.key"));

    harness.start_runner(&fixture_mount);
    wait_for_runner_health(&https_client(), harness.runner_host_port());

    let app_a_helper_tables = harness.exec_psql(
        "app_a",
        "SELECT string_agg(table_name, ',' ORDER BY table_name)
         FROM information_schema.tables
         WHERE table_schema = '_cockroach_migration_tool';",
    );
    assert_eq!(
        app_a_helper_tables.trim(),
        "app-a__public__customers,app-a__public__orders,stream_state,table_sync_state"
    );

    let app_b_helper_tables = harness.exec_psql(
        "app_b",
        "SELECT string_agg(table_name, ',' ORDER BY table_name)
         FROM information_schema.tables
         WHERE table_schema = '_cockroach_migration_tool';",
    );
    assert_eq!(
        app_b_helper_tables.trim(),
        "app-b__public__invoices,stream_state,table_sync_state"
    );
}
