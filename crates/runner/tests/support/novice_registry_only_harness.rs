use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
    time::{SystemTime, UNIX_EPOCH},
};

use reqwest::{Certificate, Identity, blocking::Client};
use serde::Deserialize;
use serde_json::json;
use tempfile::TempDir;

use crate::published_image_refs_support::{
    runner_image_ref, setup_sql_image_ref, verify_image_ref,
};
use crate::readme_operator_workspace_support::ReadmeOperatorWorkspace;

pub struct CommandOutput {
    pub stdout: String,
    pub stderr: String,
}

pub struct NoviceRegistryOnlyHarness {
    workspace: TempDir,
    readme_contract: ReadmeOperatorWorkspace,
}

pub struct RunningRunner {
    postgres_container_name: String,
    runner_container_name: String,
    host_port: u16,
    server_cert_path: PathBuf,
}

pub struct RunningDestinationPostgres {
    container_name: String,
    host_port: u16,
}

pub struct RunningVerifyCompose {
    root_dir: PathBuf,
    project_name: String,
    verify_image: String,
    verify_https_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct VerifyComposeJobResponse {
    pub job_id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyComposeErrorResponse {
    pub error: String,
}

impl NoviceRegistryOnlyHarness {
    pub fn start() -> Self {
        let workspace = tempfile::tempdir().expect("novice workspace temp dir should be created");
        let readme_contract = ReadmeOperatorWorkspace::load();
        let harness = Self {
            workspace,
            readme_contract,
        };
        harness
            .readme_contract
            .materialize_operator_workspace(harness.root_dir());
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
        let postgres = self.start_runner_destination_postgres();
        let runner_container_name = format!("cockroach-migrate-novice-runner-{}", unique_suffix());
        let host_port = pick_unused_port();
        let config_mount = format!("{}:/config:ro", self.root_dir().join("config").display());
        self.write_runner_config(
            "host.docker.internal",
            postgres.host_port(),
            "runner-secret-a",
        );
        let postgres_container_name = postgres.container_name.clone();
        std::mem::forget(postgres);

        run_command_capture(
            Command::new("docker").args([
                "run",
                "-d",
                "--name",
                &runner_container_name,
                "--add-host",
                "host.docker.internal:host-gateway",
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
            postgres_container_name,
            runner_container_name,
            host_port,
            server_cert_path: self.root_dir().join("config/certs/server.crt"),
        }
    }

    pub fn start_runner_destination_postgres(&self) -> RunningDestinationPostgres {
        let container_name = format!("cockroach-migrate-novice-postgres-{}", unique_suffix());
        let host_port = pick_unused_port();
        run_command_capture(
            Command::new("docker").args([
                "run",
                "-d",
                "--name",
                &container_name,
                "-p",
                &format!("{host_port}:5432"),
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
        wait_for_postgres(&container_name);
        enable_postgres_ssl(&container_name);
        prepare_postgres_schema(&container_name);
        RunningDestinationPostgres {
            container_name,
            host_port,
        }
    }

    pub fn run_runner_readme_runtime_failure(
        &self,
        destination_host: &str,
        destination_port: u16,
        destination_password: &str,
    ) -> CommandOutput {
        self.write_runner_config(destination_host, destination_port, destination_password);
        let config_mount = format!("{}:/config:ro", self.root_dir().join("config").display());
        let output = Command::new("docker")
            .args([
                "run",
                "--rm",
                "--add-host",
                "host.docker.internal:host-gateway",
                "-v",
                &config_mount,
                runner_image_ref(),
                "run",
                "--log-format",
                "json",
                "--config",
                "/config/runner.yml",
            ])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker run runner failure probe should start: {error}")
            });
        assert!(
            !output.status.success(),
            "docker run runner failure probe must fail for the negative config case\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        CommandOutput {
            stdout: String::from_utf8(output.stdout).expect("command stdout should be utf-8"),
            stderr: String::from_utf8(output.stderr).expect("command stderr should be utf-8"),
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

        copy_file(&setup_sql_fixture("ca.crt"), &config_dir.join("ca.crt"));
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
        copy_file(
            &investigation_cert("ca.crt"),
            &certs_dir.join("destination-ca.crt"),
        );
        copy_file(
            &investigation_cert("server.crt"),
            &certs_dir.join("destination-client.crt"),
        );
        copy_file(
            &investigation_cert("server.key"),
            &certs_dir.join("destination-client.key"),
        );
        self.write_runner_config("host.docker.internal", 5432, "runner-secret-a");
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
        generate_verify_listener_client_identity(&certs_dir);
        copy_file(
            &investigation_cert("server.crt"),
            &certs_dir.join("server.crt"),
        );
        copy_file(
            &investigation_cert("server.key"),
            &certs_dir.join("server.key"),
        );
    }

    fn root_dir(&self) -> &Path {
        self.workspace.path()
    }

    pub fn verify_compose_artifact_path(&self) -> PathBuf {
        self.root_dir().join("verify.compose.yml")
    }

    fn write_runner_config(
        &self,
        destination_host: &str,
        destination_port: u16,
        destination_password: &str,
    ) {
        let config_text = self
            .readme_contract
            .operator_file("config/runner.yml")
            .replace("pg-a.example.internal", destination_host)
            .replacen("port: 5432", &format!("port: {destination_port}"), 1)
            .replace("runner-secret-a", destination_password);
        fs::write(self.root_dir().join("config/runner.yml"), config_text)
            .expect("novice runner config should be written");
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

impl RunningDestinationPostgres {
    pub fn host_port(&self) -> u16 {
        self.host_port
    }
}

impl Drop for RunningDestinationPostgres {
    fn drop(&mut self) {
        cleanup_if_present(
            Command::new("docker").args(["container", "inspect", &self.container_name]),
            Command::new("docker").args(["rm", "-f", &self.container_name]),
            "docker rm novice postgres container",
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

    pub fn shutdown(&self) {
        run_command_output(
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
                    "down",
                    "--remove-orphans",
                ]),
            "docker compose down verify",
        );
    }

    pub fn readiness_probe_status(&self) -> u16 {
        let client = self.client();
        let mut last_error = None;
        for _ in 0..30 {
            if !container_running(
                run_command_capture(
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
                )
                .trim(),
            ) {
                panic!(
                    "verify compose runtime exited before readiness probe succeeded\n{}",
                    compose_logs(
                        &self.root_dir,
                        &self.project_name,
                        "verify.compose.yml",
                        "verify"
                    ),
                );
            }
            match client
                .get(format!(
                    "https://localhost:{}/jobs/readiness-probe",
                    self.verify_https_port
                ))
                .send()
            {
                Ok(response) => return response.status().as_u16(),
                Err(error) => {
                    last_error = Some(error.to_string());
                    thread::sleep(Duration::from_secs(1));
                }
            }
        }

        panic!(
            "verify compose readiness probe should respond over HTTPS\nlast error: {}\n{}",
            last_error.unwrap_or_else(|| "missing reqwest error".to_owned()),
            compose_logs(
                &self.root_dir,
                &self.project_name,
                "verify.compose.yml",
                "verify"
            ),
        );
    }

    pub fn start_job_with_flat_filters(
        &self,
        include_schema: &str,
        include_table: &str,
    ) -> VerifyComposeJobResponse {
        let response = self
            .client()
            .post(format!("https://localhost:{}/jobs", self.verify_https_port))
            .header("content-type", "application/json")
            .body(
                json!({
                    "include_schema": include_schema,
                    "include_table": include_table,
                })
                .to_string(),
            )
            .send()
            .unwrap_or_else(|error| panic!("verify compose POST /jobs should succeed: {error}"));
        let status = response.status();
        let body = response
            .text()
            .unwrap_or_else(|error| panic!("verify compose start response should read: {error}"));
        assert_eq!(
            status,
            reqwest::StatusCode::ACCEPTED,
            "verify compose POST /jobs should accept the flat contract\nbody:\n{}\n{}",
            body,
            compose_logs(
                &self.root_dir,
                &self.project_name,
                "verify.compose.yml",
                "verify"
            ),
        );
        parse_json(&body, "verify compose start response")
    }

    pub fn start_job_with_legacy_filters_error(&self) -> VerifyComposeErrorResponse {
        let response = self
            .client()
            .post(format!("https://localhost:{}/jobs", self.verify_https_port))
            .header("content-type", "application/json")
            .body(
                json!({
                    "filters": {
                        "include": {
                            "schema": "^public$",
                        }
                    }
                })
                .to_string(),
            )
            .send()
            .unwrap_or_else(|error| {
                panic!("verify compose POST /jobs legacy request should return a validation error: {error}")
            });
        let status = response.status();
        let body = response.text().unwrap_or_else(|error| {
            panic!("verify compose validation-error response should read: {error}")
        });
        assert_eq!(
            status,
            reqwest::StatusCode::BAD_REQUEST,
            "verify compose legacy request should fail validation\nbody:\n{}\n{}",
            body,
            compose_logs(
                &self.root_dir,
                &self.project_name,
                "verify.compose.yml",
                "verify"
            ),
        );
        parse_json(&body, "verify compose validation-error response")
    }

    pub fn wait_for_terminal_job(&self, job_id: &str) -> VerifyComposeJobResponse {
        for _ in 0..30 {
            let response = self
                .client()
                .get(format!(
                    "https://localhost:{}/jobs/{job_id}",
                    self.verify_https_port
                ))
                .send()
                .unwrap_or_else(|error| {
                    panic!("verify compose GET /jobs/{job_id} should succeed: {error}")
                });
            let status = response.status();
            let body = response
                .text()
                .unwrap_or_else(|error| panic!("verify compose job response should read: {error}"));
            assert_eq!(
                status,
                reqwest::StatusCode::OK,
                "verify compose GET /jobs/{job_id} should return job status\nbody:\n{}\n{}",
                body,
                compose_logs(
                    &self.root_dir,
                    &self.project_name,
                    "verify.compose.yml",
                    "verify"
                ),
            );
            let payload: VerifyComposeJobResponse =
                parse_json(&body, "verify compose job response");
            if payload.status != "running" {
                return payload;
            }
            thread::sleep(Duration::from_secs(1));
        }

        panic!(
            "verify compose job `{job_id}` did not reach a terminal state\n{}",
            compose_logs(
                &self.root_dir,
                &self.project_name,
                "verify.compose.yml",
                "verify"
            ),
        );
    }

    fn client(&self) -> Client {
        let certs_dir = self.root_dir.join("config/certs");
        let trusted_server_ca = fs::read(certs_dir.join("source-ca.crt"))
            .unwrap_or_else(|error| panic!("verify compose server CA should be readable: {error}"));
        Client::builder()
            .add_root_certificate(
                Certificate::from_pem(&trusted_server_ca)
                    .expect("verify compose server CA should parse"),
            )
            .identity(client_identity(
                certs_dir.join("source-client.crt").as_path(),
                certs_dir.join("source-client.key").as_path(),
            ))
            .build()
            .expect("verify compose client should build")
    }
}

fn parse_json<T>(raw: &str, description: &str) -> T
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(raw)
        .unwrap_or_else(|error| panic!("{description} should be valid json: {error}\nraw:\n{raw}"))
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

fn generate_verify_listener_client_identity(certs_dir: &Path) {
    let client_cert_config_path = certs_dir.join("source-client.cnf");
    fs::write(
        &client_cert_config_path,
        "[req]\n\
         distinguished_name = dn\n\
         prompt = no\n\
         req_extensions = req_ext\n\
         \n\
         [dn]\n\
         CN = verify-listener-client\n\
         \n\
         [req_ext]\n\
         basicConstraints = CA:FALSE\n\
         keyUsage = critical,digitalSignature,keyEncipherment\n\
         extendedKeyUsage = clientAuth\n",
    )
    .expect("verify source client config should be written");
    run_command_capture(
        Command::new("openssl").args([
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            certs_dir
                .join("source-client.key")
                .to_str()
                .expect("verify source client key path should be utf-8"),
            "-out",
            certs_dir
                .join("source-client.csr")
                .to_str()
                .expect("verify source client csr path should be utf-8"),
            "-config",
            client_cert_config_path
                .to_str()
                .expect("verify source client config path should be utf-8"),
        ]),
        "openssl req verify listener client csr",
    );
    run_command_capture(
        Command::new("openssl").args([
            "x509",
            "-req",
            "-days",
            "365",
            "-in",
            certs_dir
                .join("source-client.csr")
                .to_str()
                .expect("verify source client csr path should be utf-8"),
            "-CA",
            investigation_cert("ca.crt")
                .to_str()
                .expect("verify investigation ca cert path should be utf-8"),
            "-CAkey",
            investigation_cert("ca.key")
                .to_str()
                .expect("verify investigation ca key path should be utf-8"),
            "-CAcreateserial",
            "-CAserial",
            certs_dir
                .join("source-client.srl")
                .to_str()
                .expect("verify source client serial path should be utf-8"),
            "-out",
            certs_dir
                .join("source-client.crt")
                .to_str()
                .expect("verify source client cert path should be utf-8"),
            "-extensions",
            "req_ext",
            "-extfile",
            client_cert_config_path
                .to_str()
                .expect("verify source client config path should be utf-8"),
        ]),
        "openssl x509 verify listener client cert",
    );
}

fn copy_file_into_container(source: &Path, container: &str, destination: &str, context: &str) {
    run_command_capture(
        Command::new("docker").args([
            "cp",
            source.to_str().expect("copy source path should be utf-8"),
            &format!("{container}:{destination}"),
        ]),
        context,
    );
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

fn client_identity(cert_path: &Path, key_path: &Path) -> Identity {
    let cert = fs::read(cert_path).unwrap_or_else(|error| {
        panic!(
            "verify compose client cert should read from `{}`: {error}",
            cert_path.display(),
        )
    });
    let key = fs::read(key_path).unwrap_or_else(|error| {
        panic!(
            "verify compose client key should read from `{}`: {error}",
            key_path.display(),
        )
    });
    let mut pem = cert;
    pem.extend_from_slice(&key);
    Identity::from_pem(&pem).expect("verify compose client identity should parse")
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

fn enable_postgres_ssl(container_name: &str) {
    copy_file_into_container(
        investigation_cert("server.crt").as_path(),
        container_name,
        "/var/lib/postgresql/data/server.crt",
        "docker cp novice postgres server cert",
    );
    copy_file_into_container(
        investigation_cert("server.key").as_path(),
        container_name,
        "/var/lib/postgresql/data/server.key",
        "docker cp novice postgres server key",
    );
    run_command_capture(
        Command::new("docker").args([
            "exec",
            "-u",
            "0",
            container_name,
            "bash",
            "-lc",
            "set -euo pipefail\n\
             chown postgres:postgres /var/lib/postgresql/data/server.crt /var/lib/postgresql/data/server.key\n\
             chmod 600 /var/lib/postgresql/data/server.key\n\
             printf '\\nssl=on\\nssl_cert_file='\"'\"'/var/lib/postgresql/data/server.crt'\"'\"'\\nssl_key_file='\"'\"'/var/lib/postgresql/data/server.key'\"'\"'\\n' >> /var/lib/postgresql/data/postgresql.conf",
        ]),
        "docker exec novice postgres enable ssl",
    );
    run_command_capture(
        Command::new("docker").args(["restart", container_name]),
        "docker restart novice postgres after ssl enable",
    );
    wait_for_postgres(container_name);
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

pub fn pick_unused_port_for_tests() -> u16 {
    pick_unused_port()
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
