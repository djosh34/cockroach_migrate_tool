use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
};

use reqwest::{Certificate, Identity, blocking::Client};
use serde::Deserialize;
use serde_json::json;
use tempfile::TempDir;

use crate::e2e_harness::{
    investigation_ca_cert_path, investigation_server_cert_path, investigation_server_key_path,
    read_file,
};
use crate::e2e_integrity::{VerifyCorrectnessAudit, VerifyJobResponse};

pub struct VerifyImageHarness {
    _private: (),
}

pub struct VerifyImageRun {
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
        Self { _private: () }
    }

    pub fn run_correctness_audit(&self, run: &VerifyImageRun) -> VerifyCorrectnessAudit {
        let runtime_files = VerifyRuntimeFiles::materialize(run);
        let mut runtime = RunningVerifyImage::start(run, &runtime_files);
        runtime.wait_until_ready();
        let job_id = runtime.start_job(run);
        let response = runtime.wait_for_job(&job_id);
        VerifyCorrectnessAudit::new(run.expected_tables.clone(), response)
    }
}

struct VerifyRuntimeFiles {
    _temp_dir: TempDir,
    root_dir: PathBuf,
    listener_port: u16,
}

impl VerifyRuntimeFiles {
    fn materialize(run: &VerifyImageRun) -> Self {
        let temp_dir = tempfile::tempdir().expect("verify runtime temp dir should be created");
        let root_dir = temp_dir.path().to_path_buf();
        let listener_port = pick_unused_port();
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
  bind_addr: 127.0.0.1:{port}
  tls:
    cert_path: {server_cert_path}
    key_path: {server_key_path}
    client_ca_path: {source_ca_cert_path}
verify:
  source:
    url: {source_url}
    tls:
      ca_cert_path: {source_ca_cert_path}
      client_cert_path: {source_client_cert_path}
      client_key_path: {source_client_key_path}
  destination:
    url: {destination_url}
"#,
                port = listener_port,
                server_cert_path = certs_dir.join("server.crt").display(),
                server_key_path = certs_dir.join("server.key").display(),
                source_ca_cert_path = certs_dir.join("source-ca.crt").display(),
                source_client_cert_path = certs_dir.join("source-client.crt").display(),
                source_client_key_path = certs_dir.join("source-client.key").display(),
                source_url = run.source_url,
                destination_url = run.destination_url,
            ),
        )
        .expect("verify runtime config should be written");

        Self {
            _temp_dir: temp_dir,
            root_dir,
            listener_port,
        }
    }
}

struct RunningVerifyImage {
    process: Child,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    base_url: String,
    client: Client,
}

impl RunningVerifyImage {
    fn start(_run: &VerifyImageRun, files: &VerifyRuntimeFiles) -> Self {
        let stdout_path = files.root_dir.join("verify-service.stdout.log");
        let stderr_path = files.root_dir.join("verify-service.stderr.log");
        let stdout = fs::File::create(&stdout_path).expect("verify stdout log should open");
        let stderr = fs::File::create(&stderr_path).expect("verify stderr log should open");
        let process = Command::new("molt")
            .args([
                "verify-service",
                "run",
                "--config",
                files
                    .root_dir
                    .join("verify-service.yml")
                    .to_str()
                    .expect("verify config path should be utf-8"),
            ])
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .expect("molt verify-service should start");
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
            process,
            stdout_path,
            stderr_path,
            base_url: format!("https://127.0.0.1:{}", files.listener_port),
            client: Client::builder()
                .add_root_certificate(trusted_ca)
                .identity(client_identity)
                .build()
                .expect("verify runtime client should build"),
        }
    }

    fn wait_until_ready(&mut self) {
        for _ in 0..60 {
            self.assert_process_alive();
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
            self.logs(),
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
            self.logs(),
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

    fn wait_for_job(&mut self, job_id: &str) -> VerifyJobResponse {
        for _ in 0..120 {
            self.assert_process_alive();
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
                self.logs(),
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
            self.logs(),
        );
    }

    fn assert_process_alive(&mut self) {
        if let Some(status) = self
            .process
            .try_wait()
            .expect("verify process status should be readable")
        {
            panic!(
                "verify runtime exited early with status {status}\n{}",
                self.logs()
            );
        }
    }

    fn logs(&self) -> String {
        format!(
            "stdout:\n{}\n\nstderr:\n{}",
            read_file(&self.stdout_path),
            read_file(&self.stderr_path),
        )
    }
}

impl Drop for RunningVerifyImage {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[derive(Debug, Deserialize)]
struct VerifyStartResponse {
    job_id: String,
}

fn pick_unused_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("ephemeral verify runtime port should bind")
        .local_addr()
        .expect("ephemeral verify runtime listener should have an address")
        .port()
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
