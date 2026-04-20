use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use reqwest::{Certificate, Identity, blocking::Client};
use serde::Deserialize;
use serde_json::json;
use tempfile::TempDir;

use crate::e2e_harness::{
    investigation_ca_cert_path, investigation_server_cert_path, investigation_server_key_path,
};
use crate::e2e_integrity::{VerifyCorrectnessAudit, VerifyJobResponse};

pub struct VerifyImageHarness {
    image_tag: String,
}

pub struct VerifyImageRun {
    pub network_name: String,
    pub source_url: String,
    pub source_ca_cert_path: PathBuf,
    pub source_client_cert_path: PathBuf,
    pub source_client_key_path: PathBuf,
    pub destination_url: String,
    pub destination_ca_cert_path: PathBuf,
    pub include_schema_pattern: String,
    pub include_table_pattern: String,
    pub expected_tables: Vec<String>,
}

impl VerifyImageHarness {
    pub fn start() -> Self {
        let harness = Self {
            image_tag: format!("cockroach-migrate-verify-test-{}", unique_suffix()),
        };
        harness.build_verify_image();
        harness
    }

    pub fn run_correctness_audit(&self, run: &VerifyImageRun) -> VerifyCorrectnessAudit {
        let runtime_files = VerifyRuntimeFiles::materialize(run);
        let runtime = RunningVerifyImage::start(&self.image_tag, run, &runtime_files);
        runtime.wait_until_ready();
        let job_id = runtime.start_job(run);
        let response = runtime.wait_for_job(&job_id);
        VerifyCorrectnessAudit::new(run.expected_tables.clone(), response)
    }

    fn build_verify_image(&self) {
        run_command_capture(
            Command::new("docker").args(docker_build_image_args(&self.image_tag)),
            "docker build verify image",
        );
    }
}

struct VerifyRuntimeFiles {
    _temp_dir: TempDir,
    root_dir: PathBuf,
}

impl VerifyRuntimeFiles {
    fn materialize(run: &VerifyImageRun) -> Self {
        let temp_dir = tempfile::tempdir().expect("verify runtime temp dir should be created");
        let root_dir = temp_dir.path().to_path_buf();
        let certs_dir = root_dir.join("certs");
        fs::create_dir_all(&certs_dir).expect("verify runtime cert dir should be created");

        copy_cert(
            &run.source_ca_cert_path,
            &certs_dir.join("source-ca.crt"),
            "verify runtime source ca",
        );
        copy_cert(
            &run.source_client_cert_path,
            &certs_dir.join("source-client.crt"),
            "verify runtime source client cert",
        );
        copy_cert(
            &run.source_client_key_path,
            &certs_dir.join("source-client.key"),
            "verify runtime source client key",
        );
        copy_cert(
            &run.destination_ca_cert_path,
            &certs_dir.join("destination-ca.crt"),
            "verify runtime destination ca",
        );
        copy_cert(
            &investigation_server_cert_path(),
            &certs_dir.join("server.crt"),
            "verify runtime listener server cert",
        );
        copy_cert(
            &investigation_server_key_path(),
            &certs_dir.join("server.key"),
            "verify runtime listener server key",
        );

        let config_path = root_dir.join("verify-service.yml");
        fs::write(
            &config_path,
            format!(
                r#"listener:
  bind_addr: 0.0.0.0:8080
  tls:
    cert_path: /work/config/certs/server.crt
    key_path: /work/config/certs/server.key
    client_ca_path: /work/config/certs/source-ca.crt
verify:
  source:
    url: {source_url}
    ca_cert_path: /work/config/certs/source-ca.crt
    client_cert_path: /work/config/certs/source-client.crt
    client_key_path: /work/config/certs/source-client.key
  destination:
    url: {destination_url}
    ca_cert_path: /work/config/certs/destination-ca.crt
"#,
                source_url = run.source_url,
                destination_url = run.destination_url,
            ),
        )
        .expect("verify runtime config should be written");

        Self {
            _temp_dir: temp_dir,
            root_dir,
        }
    }
}

struct RunningVerifyImage {
    container_name: String,
    base_url: String,
    client: Client,
}

impl RunningVerifyImage {
    fn start(image_tag: &str, run: &VerifyImageRun, files: &VerifyRuntimeFiles) -> Self {
        let container_name = format!("cockroach-migrate-verify-runtime-{}", unique_suffix());
        let host_port = pick_unused_port();
        let config_mount = format!("{}:/work/config:ro", files.root_dir.display());
        run_command_capture(
            Command::new("docker").args([
                "run",
                "-d",
                "--rm",
                "--name",
                &container_name,
                "--network",
                &run.network_name,
                "-p",
                &format!("127.0.0.1:{host_port}:8080"),
                "-v",
                &config_mount,
                image_tag,
                "--config",
                "/work/config/verify-service.yml",
            ]),
            "docker run verify image runtime",
        );
        let trusted_ca = Certificate::from_pem(
            &fs::read(investigation_ca_cert_path())
                .expect("verify runtime investigation ca should be readable"),
        )
        .expect("verify runtime investigation ca should parse");
        let client_identity = client_identity(
            files
                .root_dir
                .join("certs")
                .join("source-client.crt")
                .as_path(),
            files
                .root_dir
                .join("certs")
                .join("source-client.key")
                .as_path(),
        );
        Self {
            container_name,
            base_url: format!("https://127.0.0.1:{host_port}"),
            client: Client::builder()
                .add_root_certificate(trusted_ca)
                .identity(client_identity)
                .build()
                .expect("verify runtime client should build"),
        }
    }

    fn wait_until_ready(&self) {
        for _ in 0..60 {
            self.assert_container_alive();
            match self
                .client
                .get(format!("{}/jobs/readiness-probe", self.base_url))
                .send()
            {
                Ok(response) if response.status() == reqwest::StatusCode::NOT_FOUND => return,
                Ok(_) | Err(_) => thread::sleep(std::time::Duration::from_secs(1)),
            }
        }

        panic!(
            "verify image runtime did not become ready on {}\n{}",
            self.base_url,
            docker_logs(&self.container_name),
        );
    }

    fn start_job(&self, run: &VerifyImageRun) -> String {
        let response = self
            .client
            .post(format!("{}/jobs", self.base_url))
            .header("content-type", "application/json")
            .body(
                json!({
                    "include_schema": run.include_schema_pattern,
                    "include_table": run.include_table_pattern,
                })
                .to_string(),
            )
            .send()
            .unwrap_or_else(|error| panic!("verify image POST /jobs should succeed: {error}"));
        assert!(
            response.status().is_success(),
            "verify image POST /jobs failed with status {}\n{}",
            response.status(),
            docker_logs(&self.container_name),
        );
        let payload = parse_json::<VerifyStartResponse>(
            response
                .text()
                .unwrap_or_else(|error| panic!("verify image start response should read: {error}"))
                .as_str(),
            "verify image start response",
        );
        payload.job_id
    }

    fn wait_for_job(&self, job_id: &str) -> VerifyJobResponse {
        for _ in 0..120 {
            self.assert_container_alive();
            let response = self
                .client
                .get(format!("{}/jobs/{job_id}", self.base_url))
                .send()
                .unwrap_or_else(|error| {
                    panic!("verify image GET /jobs/{job_id} should succeed: {error}")
                });
            assert!(
                response.status().is_success(),
                "verify image GET /jobs/{job_id} failed with status {}\n{}",
                response.status(),
                docker_logs(&self.container_name),
            );
            let payload = parse_json::<VerifyJobResponse>(
                response
                    .text()
                    .unwrap_or_else(|error| {
                        panic!("verify image job response should read: {error}")
                    })
                    .as_str(),
                "verify image job response",
            );
            if !payload.is_running() {
                return payload;
            }
            thread::sleep(std::time::Duration::from_secs(1));
        }

        panic!(
            "verify image job `{job_id}` did not finish in time\n{}",
            docker_logs(&self.container_name),
        );
    }

    fn assert_container_alive(&self) {
        if !container_running(&self.container_name) {
            panic!(
                "verify image runtime container exited early\n{}",
                docker_logs(&self.container_name),
            );
        }
    }
}

impl Drop for RunningVerifyImage {
    fn drop(&mut self) {
        let output = Command::new("docker")
            .args(["container", "inspect", &self.container_name])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker inspect verify runtime container should start: {error}")
            });
        if output.status.success() {
            run_command_capture(
                Command::new("docker").args(["rm", "-f", &self.container_name]),
                "docker rm verify runtime container",
            );
        }
    }
}

impl Drop for VerifyImageHarness {
    fn drop(&mut self) {
        let output = Command::new("docker")
            .args(["image", "inspect", &self.image_tag])
            .output()
            .unwrap_or_else(|error| {
                panic!("docker image rm verify image probe should start: {error}")
            });
        if output.status.success() {
            run_command_capture(
                Command::new("docker").args(["image", "rm", "-f", &self.image_tag]),
                "docker image rm verify image",
            );
        }
    }
}

#[derive(Debug, Deserialize)]
struct VerifyStartResponse {
    job_id: String,
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
    TcpListener::bind("127.0.0.1:0")
        .expect("ephemeral verify runtime port should bind")
        .local_addr()
        .expect("ephemeral verify runtime listener should have an address")
        .port()
}

fn run_command_capture(command: &mut Command, context: &str) -> String {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("{context} should start: {error}"));
    assert!(
        output.status.success(),
        "{context} failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("command stdout should be utf-8")
}

fn copy_cert(source: &Path, destination: &Path, description: &str) {
    fs::copy(source, destination).unwrap_or_else(|error| {
        panic!(
            "{description} should copy from `{}` to `{}`: {error}",
            source.display(),
            destination.display(),
        )
    });
}

fn client_identity(cert_path: &Path, key_path: &Path) -> Identity {
    let cert = fs::read(cert_path).unwrap_or_else(|error| {
        panic!(
            "verify runtime client cert should read from `{}`: {error}",
            cert_path.display(),
        )
    });
    let key = fs::read(key_path).unwrap_or_else(|error| {
        panic!(
            "verify runtime client key should read from `{}`: {error}",
            key_path.display(),
        )
    });
    let mut pem = cert;
    pem.extend_from_slice(&key);
    Identity::from_pem(&pem).expect("verify runtime client identity should parse")
}

fn parse_json<T>(raw: &str, description: &str) -> T
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(raw)
        .unwrap_or_else(|error| panic!("{description} should parse from JSON `{raw}`: {error}"))
}

fn container_running(container: &str) -> bool {
    let output = Command::new("docker")
        .args([
            "container",
            "inspect",
            "-f",
            "{{.State.Running}}",
            container,
        ])
        .output()
        .unwrap_or_else(|error| {
            panic!("docker inspect verify runtime container should start: {error}")
        });
    output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
}

fn docker_logs(container: &str) -> String {
    let output = Command::new("docker")
        .args(["logs", container])
        .output()
        .unwrap_or_else(|error| {
            panic!("docker logs verify runtime container should start: {error}")
        });
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

fn docker_build_image_args(image_tag: &str) -> Vec<String> {
    vec![
        String::from("build"),
        String::from("-t"),
        image_tag.to_owned(),
        String::from("-f"),
        verify_slice_root().join("Dockerfile").display().to_string(),
        verify_slice_root().display().to_string(),
    ]
}

fn verify_slice_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("cockroachdb_molt/molt")
}
