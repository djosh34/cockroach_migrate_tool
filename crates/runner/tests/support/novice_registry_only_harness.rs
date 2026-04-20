use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

use tempfile::TempDir;

use crate::published_image_refs_support::{
    runner_image_ref, setup_sql_image_ref, verify_image_ref,
};

pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
}

pub struct NoviceRegistryOnlyHarness {
    workspace: TempDir,
}

pub struct RunningRunner {
    network_name: String,
    postgres_container_name: String,
    runner_container_name: String,
    host_port: u16,
    server_cert_path: PathBuf,
}

pub struct RunningVerifyCompose {
    root_dir: PathBuf,
    project_name: String,
    verify_image: String,
    verify_https_port: u16,
}

impl NoviceRegistryOnlyHarness {
    pub fn start() -> Self {
        let workspace = tempfile::tempdir().expect("novice workspace temp dir should be created");
        let harness = Self { workspace };
        harness.materialize_setup_sql_workspace();
        harness.materialize_runner_workspace();
        harness.materialize_verify_workspace();
        harness
    }

    pub fn run_setup_sql_compose_emit_cockroach_sql(&self) -> String {
        let image_ref = setup_sql_image_ref();
        let output = Command::new("docker")
            .current_dir(self.root_dir())
            .env("SETUP_SQL_IMAGE", image_ref)
            .args([
                "compose",
                "-f",
                "setup-sql.compose.yml",
                "run",
                "--rm",
                "setup-sql",
            ])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker compose run setup-sql should start: {error}");
            });
        assert!(
            output.status.success(),
            "docker compose run setup-sql failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        String::from_utf8(output.stdout).expect("setup-sql compose stdout should be utf-8")
    }

    pub fn run_runner_readme_validate_config(&self) -> CommandOutput {
        let config_mount = format!("{}:/config:ro", self.root_dir().join("config").display());
        run_command_output(
            Command::new("docker").args([
                "run",
                "--rm",
                "-v",
                &config_mount,
                runner_image_ref(),
                "validate-config",
                "--log-format",
                "json",
                "--config",
                "/config/runner.yml",
            ]),
            "docker run runner validate-config",
        )
    }

    pub fn start_runner_readme_runtime(&self) -> RunningRunner {
        let network_name = format!("cockroach-migrate-novice-net-{}", unique_suffix());
        let postgres_container_name =
            format!("cockroach-migrate-novice-postgres-{}", unique_suffix());
        let runner_container_name = format!("cockroach-migrate-novice-runner-{}", unique_suffix());
        let host_port = pick_unused_port();
        let config_mount = format!("{}:/config:ro", self.root_dir().join("config").display());

        run_command_capture(
            Command::new("docker").args(["network", "create", &network_name]),
            "docker network create novice runner network",
        );

        run_command_capture(
            Command::new("docker").args([
                "run",
                "-d",
                "--name",
                &postgres_container_name,
                "--network",
                &network_name,
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
            "docker run novice postgres",
        );
        wait_for_postgres(&postgres_container_name);
        prepare_postgres_schema(&postgres_container_name);

        run_command_capture(
            Command::new("docker").args([
                "run",
                "-d",
                "--name",
                &runner_container_name,
                "--network",
                &network_name,
                "-p",
                &format!("127.0.0.1:{host_port}:8443"),
                "-v",
                &config_mount,
                runner_image_ref(),
                "run",
                "--log-format",
                "json",
                "--config",
                "/config/runner.yml",
            ]),
            "docker run novice runner runtime",
        );

        RunningRunner {
            network_name,
            postgres_container_name,
            runner_container_name,
            host_port,
            server_cert_path: self.root_dir().join("config/certs/server.crt"),
        }
    }

    pub fn run_setup_sql_compose_emit_postgres_grants(&self) -> String {
        let output = Command::new("docker")
            .current_dir(self.root_dir())
            .env("SETUP_SQL_IMAGE", setup_sql_image_ref())
            .args([
                "compose",
                "-f",
                "setup-sql.compose.yml",
                "run",
                "--rm",
                "setup-sql",
                "emit-postgres-grants",
                "--log-format",
                "json",
                "--config",
                "/config/postgres-grants.yml",
            ])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker compose run setup-sql emit-postgres-grants should start: {error}");
            });
        assert!(
            output.status.success(),
            "docker compose run setup-sql emit-postgres-grants failed with status {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        String::from_utf8(output.stdout)
            .expect("setup-sql compose postgres grants stdout should be utf-8")
    }

    pub fn run_runner_compose_validate_config(&self) -> CommandOutput {
        run_command_output(
            Command::new("docker")
                .current_dir(self.root_dir())
                .env("RUNNER_IMAGE", runner_image_ref())
                .args([
                    "compose",
                    "-f",
                    "runner.compose.yml",
                    "run",
                    "--rm",
                    "runner",
                    "validate-config",
                    "--log-format",
                    "json",
                    "--config",
                    "/config/runner.yml",
                ]),
            "docker compose run runner validate-config",
        )
    }

    pub fn start_verify_compose_runtime(&self) -> RunningVerifyCompose {
        let project_name = format!("cockroach-migrate-novice-verify-{}", unique_suffix());
        let verify_https_port = pick_unused_port();
        let verify_image = verify_image_ref().to_owned();
        run_command_capture(
            Command::new("docker")
                .current_dir(self.root_dir())
                .env("VERIFY_IMAGE", &verify_image)
                .env("VERIFY_HTTPS_PORT", verify_https_port.to_string())
                .args([
                    "compose",
                    "-p",
                    &project_name,
                    "-f",
                    "verify.compose.yml",
                    "up",
                    "-d",
                    "verify",
                ]),
            "docker compose up verify",
        );

        RunningVerifyCompose {
            root_dir: self.root_dir().to_path_buf(),
            project_name,
            verify_image,
            verify_https_port,
        }
    }

    fn materialize_setup_sql_workspace(&self) {
        let config_dir = self.root_dir().join("config");
        fs::create_dir_all(&config_dir).expect("novice config dir should be created");

        copy_file(
            &setup_sql_fixture("readme-cockroach-setup-config.yml"),
            &config_dir.join("cockroach-setup.yml"),
        );
        copy_file(
            &setup_sql_fixture("valid-postgres-grants-config.yml"),
            &config_dir.join("postgres-grants.yml"),
        );
        copy_file(&setup_sql_fixture("ca.crt"), &config_dir.join("ca.crt"));
        copy_file(
            &repo_root().join("artifacts/compose/setup-sql.compose.yml"),
            &self.root_dir().join("setup-sql.compose.yml"),
        );
    }

    fn materialize_runner_workspace(&self) {
        let certs_dir = self.root_dir().join("config/certs");
        fs::create_dir_all(&certs_dir).expect("novice runner cert dir should be created");

        copy_file(
            &runner_fixture("certs/server.crt"),
            &certs_dir.join("server.crt"),
        );
        copy_file(
            &runner_fixture("certs/server.key"),
            &certs_dir.join("server.key"),
        );
        fs::write(
            self.root_dir().join("config/runner.yml"),
            r#"webhook:
  bind_addr: 0.0.0.0:8443
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
reconcile:
  interval_secs: 30
mappings:
  - id: app-a
    source:
      database: demo_a
      tables:
        - public.customers
        - public.orders
    destination:
      host: postgres
      port: 5432
      database: app_a
      user: migration_user_a
      password: runner-secret-a
"#,
        )
        .expect("novice runner config should be written");
        copy_file(
            &repo_root().join("artifacts/compose/runner.compose.yml"),
            &self.root_dir().join("runner.compose.yml"),
        );
    }

    fn materialize_verify_workspace(&self) {
        let certs_dir = self.root_dir().join("config/certs");
        fs::create_dir_all(&certs_dir).expect("novice verify cert dir should be created");

        copy_file(
            &investigation_cert("ca.crt"),
            &certs_dir.join("source-ca.crt"),
        );
        copy_file(
            &investigation_cert("ca.crt"),
            &certs_dir.join("destination-ca.crt"),
        );
        copy_file(
            &investigation_cert("ca.crt"),
            &certs_dir.join("client-ca.crt"),
        );
        copy_file(
            &investigation_cert("server.crt"),
            &certs_dir.join("source-client.crt"),
        );
        copy_file(
            &investigation_cert("server.key"),
            &certs_dir.join("source-client.key"),
        );
        copy_file(
            &investigation_cert("server.crt"),
            &certs_dir.join("server.crt"),
        );
        copy_file(
            &investigation_cert("server.key"),
            &certs_dir.join("server.key"),
        );
        fs::write(
            self.root_dir().join("config/verify-service.yml"),
            r#"listener:
  bind_addr: 0.0.0.0:8080
  transport:
    mode: https
  tls:
    cert_path: /config/certs/server.crt
    key_path: /config/certs/server.key
    client_auth:
      mode: mtls
      client_ca_path: /config/certs/client-ca.crt
verify:
  source:
    url: postgresql://verify_source@source.internal:5432/appdb
    tls:
      mode: verify-ca
      ca_cert_path: /config/certs/source-ca.crt
  destination:
    url: postgresql://verify_target@destination.internal:5432/appdb
    tls:
      mode: verify-ca
      ca_cert_path: /config/certs/destination-ca.crt
"#,
        )
        .expect("novice verify config should be written");
        copy_file(
            &repo_root().join("artifacts/compose/verify.compose.yml"),
            &self.root_dir().join("verify.compose.yml"),
        );
    }

    fn root_dir(&self) -> &Path {
        self.workspace.path()
    }
}

impl RunningRunner {
    pub fn wait_for_health(&self) {
        for _ in 0..60 {
            if !container_running(&self.runner_container_name) {
                panic!(
                    "runner novice container exited early\n{}",
                    docker_logs(&self.runner_container_name),
                );
            }
            let healthcheck = Command::new("curl")
                .args([
                    "--silent",
                    "--show-error",
                    "--fail",
                    "--cacert",
                    &self.server_cert_path.display().to_string(),
                    &format!("https://localhost:{}/healthz", self.host_port),
                ])
                .status()
                .unwrap_or_else(|error| panic!("curl healthcheck should start: {error}"));
            if healthcheck.success() {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "runner novice container did not become healthy\n{}",
            docker_logs(&self.runner_container_name),
        );
    }
}

impl Drop for RunningRunner {
    fn drop(&mut self) {
        cleanup_if_present(
            Command::new("docker").args(["container", "inspect", &self.runner_container_name]),
            Command::new("docker").args(["rm", "-f", &self.runner_container_name]),
            "docker rm novice runner container",
        );
        cleanup_if_present(
            Command::new("docker").args(["container", "inspect", &self.postgres_container_name]),
            Command::new("docker").args(["rm", "-f", &self.postgres_container_name]),
            "docker rm novice postgres container",
        );
        cleanup_if_present(
            Command::new("docker").args(["network", "inspect", &self.network_name]),
            Command::new("docker").args(["network", "rm", &self.network_name]),
            "docker network rm novice runner network",
        );
    }
}

impl RunningVerifyCompose {
    pub fn wait_until_running(&self) {
        for _ in 0..30 {
            let container_id = run_command_capture(
                Command::new("docker")
                    .current_dir(&self.root_dir)
                    .env("VERIFY_IMAGE", &self.verify_image)
                    .env("VERIFY_HTTPS_PORT", self.verify_https_port.to_string())
                    .args([
                        "compose",
                        "-p",
                        &self.project_name,
                        "-f",
                        "verify.compose.yml",
                        "ps",
                        "-q",
                        "verify",
                    ]),
                "docker compose ps verify",
            );
            let container_id = container_id.trim();
            if !container_id.is_empty() && container_running(container_id) {
                return;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "verify compose service did not stay running\n{}",
            compose_logs(
                &self.root_dir,
                &self.project_name,
                "verify.compose.yml",
                "verify"
            ),
        );
    }
}

impl Drop for RunningVerifyCompose {
    fn drop(&mut self) {
        let _ = Command::new("docker")
            .current_dir(&self.root_dir)
            .env("VERIFY_IMAGE", &self.verify_image)
            .env("VERIFY_HTTPS_PORT", self.verify_https_port.to_string())
            .args([
                "compose",
                "-p",
                &self.project_name,
                "-f",
                "verify.compose.yml",
                "down",
                "--remove-orphans",
            ])
            .output();
    }
}

fn copy_file(from: &Path, to: &Path) {
    let contents = fs::read(from)
        .unwrap_or_else(|error| panic!("fixture `{}` should be readable: {error}", from.display()));
    fs::write(to, contents).unwrap_or_else(|error| {
        panic!(
            "fixture copy `{}` should be writable: {error}",
            to.display()
        )
    });
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve")
}

fn setup_sql_fixture(name: &str) -> PathBuf {
    repo_root()
        .join("crates/setup-sql/tests/fixtures")
        .join(name)
}

fn runner_fixture(name: &str) -> PathBuf {
    repo_root().join("crates/runner/tests/fixtures").join(name)
}

fn investigation_cert(name: &str) -> PathBuf {
    repo_root()
        .join("investigations/cockroach-webhook-cdc/certs")
        .join(name)
}

fn run_command_capture(command: &mut Command, context: &str) -> String {
    run_command_output(command, context).stdout
}

fn run_command_output(command: &mut Command, context: &str) -> CommandOutput {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{context} should start: {error}"));
    assert!(
        output.status.success(),
        "{context} failed with status {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    CommandOutput {
        stdout: String::from_utf8(output.stdout).expect("command stdout should be utf-8"),
        stderr: String::from_utf8(output.stderr).expect("command stderr should be utf-8"),
    }
}

fn wait_for_postgres(container_name: &str) {
    for _ in 0..60 {
        let output = Command::new("docker")
            .args([
                "exec",
                "-e",
                "PGPASSWORD=postgres",
                container_name,
                "pg_isready",
                "-h",
                "127.0.0.1",
                "-U",
                "postgres",
                "-d",
                "postgres",
            ])
            .output()
            .unwrap_or_else(|error| panic!("docker exec pg_isready should start: {error}"));
        if output.status.success() {
            return;
        }
        thread::sleep(Duration::from_secs(1));
    }

    panic!("novice postgres container did not become ready");
}

fn prepare_postgres_schema(container_name: &str) {
    exec_psql(
        container_name,
        "postgres",
        "CREATE ROLE migration_user_a LOGIN PASSWORD 'runner-secret-a';",
    );
    exec_psql(
        container_name,
        "postgres",
        "CREATE DATABASE app_a OWNER migration_user_a;",
    );
    exec_psql(
        container_name,
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
}

fn exec_psql(container_name: &str, database: &str, sql: &str) {
    run_command_capture(
        Command::new("docker").args([
            "exec",
            "-e",
            "PGPASSWORD=postgres",
            container_name,
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
        "docker exec novice psql",
    );
}

fn container_running(container_name: &str) -> bool {
    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.State.Running}}", container_name])
        .output()
        .unwrap_or_else(|error| panic!("docker inspect `{container_name}` should start: {error}"));
    output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
}

fn docker_logs(container_name: &str) -> String {
    let output = Command::new("docker")
        .args(["logs", container_name])
        .output()
        .unwrap_or_else(|error| panic!("docker logs `{container_name}` should start: {error}"));
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

fn compose_logs(root_dir: &Path, project_name: &str, compose_file: &str, service: &str) -> String {
    let output = Command::new("docker")
        .current_dir(root_dir)
        .env("VERIFY_IMAGE", verify_image_ref())
        .args([
            "compose",
            "-p",
            project_name,
            "-f",
            compose_file,
            "logs",
            service,
        ])
        .output()
        .unwrap_or_else(|error| panic!("docker compose logs {service} should start: {error}"));
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
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

fn pick_unused_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("ephemeral port should bind")
        .local_addr()
        .expect("bound socket should have a local address")
        .port()
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
