use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::{Certificate, blocking::Client};

use crate::runner_docker_contract::{RunnerDockerContract, RunnerRuntimeLaunch};

pub struct RunnerImageHarness {
    image_tag: String,
    network_name: String,
    postgres_container: String,
    runner_container: String,
    runner_host_port: u16,
}

impl RunnerImageHarness {
    pub fn start() -> Self {
        let suffix = unique_suffix();
        let harness = Self {
            image_tag: format!("cockroach-migrate-runner-test-{suffix}"),
            network_name: format!("cockroach-migrate-runner-net-{suffix}"),
            postgres_container: format!("cockroach-migrate-postgres-{suffix}"),
            runner_container: format!("cockroach-migrate-runner-{suffix}"),
            runner_host_port: pick_unused_port(),
        };
        harness.build_runner_image();
        harness.create_network();
        harness.start_postgres();
        harness.wait_for_postgres();
        harness.prepare_postgres_schema();
        harness
    }

    pub fn image_entrypoint_json(&self) -> String {
        run_command_capture(
            Command::new("docker").args(
                RunnerDockerContract::docker_inspect_image_entrypoint_args(&self.image_tag),
            ),
            "docker image inspect",
        )
    }

    pub fn validate_mounted_config(&self) -> String {
        let fixture_mount = format!("{}:/config:ro", fixtures_dir().display());
        run_command_capture(
            Command::new("docker").args(RunnerDockerContract::docker_validate_config_args(
                &self.image_tag,
                &fixture_mount,
                "/config/container-runner-config.yml",
                Some(&self.network_name),
            )),
            "docker validate-config",
        )
    }

    pub fn start_runner_container(&self) {
        let fixture_mount = format!("{}:/config:ro", fixtures_dir().display());
        run_command_capture(
            Command::new("docker").args(RunnerDockerContract::docker_run_runtime_args(
                RunnerRuntimeLaunch {
                    image_tag: &self.image_tag,
                    container_name: &self.runner_container,
                    network_name: &self.network_name,
                    auto_remove: true,
                    host_bind_ip: Some("127.0.0.1"),
                    host_port: self.runner_host_port,
                    mounts: &[&fixture_mount],
                    extra_docker_args: &[],
                    config_path: "/config/container-runner-config.yml",
                },
            )),
            "docker run runner",
        );
    }

    pub fn wait_for_runner_health(&self) {
        let client = https_client(&fixtures_dir().join("certs").join("server.crt"));
        for _ in 0..60 {
            match client
                .get(format!(
                    "https://localhost:{}/healthz",
                    self.runner_host_port
                ))
                .send()
            {
                Ok(response) if response.status().is_success() => return,
                Ok(_) | Err(_) => thread::sleep(Duration::from_secs(1)),
            }
        }

        panic!(
            "runner did not become healthy\n{}",
            docker_logs(&self.runner_container)
        );
    }

    pub fn helper_tables(&self, database: &str) -> String {
        self.exec_psql(
            database,
            "SELECT string_agg(table_name, ',' ORDER BY table_name)
             FROM information_schema.tables
             WHERE table_schema = '_cockroach_migration_tool';",
        )
    }

    fn build_runner_image(&self) {
        run_command_capture(
            Command::new("docker")
                .args(RunnerDockerContract::docker_build_image_args(
                    &self.image_tag,
                ))
                .arg(repo_root()),
            "docker build",
        );
    }

    fn create_network(&self) {
        run_command_capture(
            Command::new("docker").args(["network", "create", &self.network_name]),
            "docker network create",
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

    fn prepare_postgres_schema(&self) {
        self.exec_psql(
            "postgres",
            "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
        );
        self.exec_psql(
            "postgres",
            "CREATE ROLE migration_user_b LOGIN PASSWORD 'runner-secret-b';",
        );
        self.exec_psql("postgres", "CREATE DATABASE app_a OWNER migration_user_a;");
        self.exec_psql("postgres", "CREATE DATABASE app_b OWNER migration_user_b;");
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
}

impl Drop for RunnerImageHarness {
    fn drop(&mut self) {
        cleanup_if_present(
            Command::new("docker").args(["container", "inspect", &self.runner_container]),
            Command::new("docker").args(["rm", "-f", &self.runner_container]),
            "docker rm runner container",
        );
        cleanup_if_present(
            Command::new("docker").args(["container", "inspect", &self.postgres_container]),
            Command::new("docker").args(["rm", "-f", &self.postgres_container]),
            "docker rm postgres container",
        );
        cleanup_if_present(
            Command::new("docker").args(["network", "inspect", &self.network_name]),
            Command::new("docker").args(["network", "rm", &self.network_name]),
            "docker network rm",
        );
        cleanup_if_present(
            Command::new("docker").args(["image", "inspect", &self.image_tag]),
            Command::new("docker").args(["image", "rm", "-f", &self.image_tag]),
            "docker image rm",
        );
    }
}

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
    static UNIQUE_SUFFIX_COUNTER: AtomicU64 = AtomicU64::new(0);

    format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos(),
        UNIQUE_SUFFIX_COUNTER.fetch_add(1, Ordering::Relaxed),
    )
}

fn pick_unused_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("ephemeral port should bind")
        .local_addr()
        .expect("bound socket should have a local address")
        .port()
}

fn run_command_capture(command: &mut Command, context: &str) -> String {
    let (stdout, _) = run_command_output(command, context);
    stdout
}

fn run_command_output(command: &mut Command, context: &str) -> (String, String) {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{context} should start: {error}"));
    assert!(
        output.status.success(),
        "{context} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    (
        String::from_utf8(output.stdout).expect("command stdout should be utf-8"),
        String::from_utf8(output.stderr).expect("command stderr should be utf-8"),
    )
}

fn cleanup_if_present(probe: &mut Command, cleanup: &mut Command, context: &str) {
    let output = probe
        .output()
        .unwrap_or_else(|error| panic!("{context} probe should start: {error}"));
    if output.status.success() {
        run_command_capture(cleanup, context);
    }
}

fn https_client(certificate_path: &PathBuf) -> Client {
    let certificate =
        Certificate::from_pem(&fs::read(certificate_path).expect("certificate should be readable"))
            .expect("certificate should parse");

    Client::builder()
        .add_root_certificate(certificate)
        .build()
        .expect("https client should build")
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
