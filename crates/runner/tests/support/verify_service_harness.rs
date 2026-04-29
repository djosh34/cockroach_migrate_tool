use std::{
    fs,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    thread,
};

use reqwest::{Certificate, Identity, Url, blocking::Client};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tempfile::TempDir;

use crate::e2e_harness::{
    investigation_ca_cert_path, investigation_server_cert_path, investigation_server_key_path,
    read_file,
};
use crate::e2e_integrity::{VerifyCorrectnessAudit, VerifyJobResponse};

pub struct VerifyServiceHarness {
    _private: (),
}

pub struct VerifyServiceRun {
    pub source_url: String,
    pub source_ca_cert_path: PathBuf,
    pub source_client_cert_path: PathBuf,
    pub source_client_key_path: PathBuf,
    pub destination_url: String,
    pub destination_ca_cert_path: PathBuf,
    pub schema_match: Vec<String>,
    pub table_match: Vec<String>,
    pub expected_tables: Vec<String>,
}

#[derive(Serialize)]
struct VerifyServiceConfig {
    listener: VerifyListenerConfig,
    verify: VerifyRuntimeVerifyConfig,
}

#[derive(Serialize)]
struct VerifyListenerConfig {
    bind_addr: String,
    tls: VerifyListenerTlsConfig,
}

#[derive(Serialize)]
struct VerifyListenerTlsConfig {
    cert_path: String,
    key_path: String,
    client_ca_path: String,
}

#[derive(Serialize)]
struct VerifyRuntimeVerifyConfig {
    source: VerifyDatabaseConfig,
    destination: VerifyDatabaseConfig,
    databases: Vec<VerifyDatabaseMappingConfig>,
}

#[derive(Serialize)]
struct VerifyDatabaseConfig {
    host: String,
    port: u16,
    username: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    password: Option<String>,
    sslmode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tls: Option<VerifyDatabaseTlsConfig>,
}

#[derive(Serialize)]
struct VerifyDatabaseMappingConfig {
    name: String,
    source_database: String,
    destination_database: String,
}

#[derive(Clone, Default, Serialize)]
struct VerifyDatabaseTlsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    ca_cert_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_cert_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    client_key_path: Option<String>,
}

struct ParsedPostgresUrl {
    host: String,
    port: u16,
    database: String,
    username: String,
    password: Option<String>,
    sslmode: String,
    tls: VerifyDatabaseTlsConfig,
}

impl VerifyServiceHarness {
    pub fn start() -> Self {
        Self { _private: () }
    }

    pub fn run_correctness_audit(&self, run: &VerifyServiceRun) -> VerifyCorrectnessAudit {
        let runtime_files = VerifyRuntimeFiles::materialize(run);
        let mut runtime = RunningVerifyService::start(run, &runtime_files);
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
    fn materialize(run: &VerifyServiceRun) -> Self {
        let temp_dir = tempfile::tempdir().expect("verify runtime temp dir should be created");
        let root_dir = temp_dir.path().to_path_buf();
        let listener_port = pick_unused_port();
        let certs_dir = root_dir.join("certs");
        fs::create_dir_all(&certs_dir).expect("verify runtime cert dir should be created");
        let source_ca_cert_path = certs_dir.join("source-ca.crt");
        let source_client_cert_path = certs_dir.join("source-client.crt");
        let source_client_key_path = certs_dir.join("source-client.key");
        let destination_ca_cert_path = certs_dir.join("destination-ca.crt");
        let server_cert_path = certs_dir.join("server.crt");
        let server_key_path = certs_dir.join("server.key");

        copy_cert(
            &run.source_ca_cert_path,
            &source_ca_cert_path,
            "verify runtime source ca",
        );
        copy_cert(
            &run.source_client_cert_path,
            &source_client_cert_path,
            "verify runtime source client cert",
        );
        copy_cert(
            &run.source_client_key_path,
            &source_client_key_path,
            "verify runtime source client key",
        );
        copy_cert(
            &run.destination_ca_cert_path,
            &destination_ca_cert_path,
            "verify runtime destination ca",
        );
        copy_cert(
            &investigation_server_cert_path(),
            &server_cert_path,
            "verify runtime listener server cert",
        );
        copy_cert(
            &investigation_server_key_path(),
            &server_key_path,
            "verify runtime listener server key",
        );

        let source = parse_postgres_url(&run.source_url).with_tls(VerifyDatabaseTlsConfig {
            ca_cert_path: Some(path_string(&source_ca_cert_path)),
            client_cert_path: Some(path_string(&source_client_cert_path)),
            client_key_path: Some(path_string(&source_client_key_path)),
        });
        let destination =
            parse_postgres_url(&run.destination_url).with_tls(VerifyDatabaseTlsConfig {
                ca_cert_path: Some(path_string(&destination_ca_cert_path)),
                client_cert_path: None,
                client_key_path: None,
            });
        let config = VerifyServiceConfig {
            listener: VerifyListenerConfig {
                bind_addr: format!("127.0.0.1:{listener_port}"),
                tls: VerifyListenerTlsConfig {
                    cert_path: path_string(&server_cert_path),
                    key_path: path_string(&server_key_path),
                    client_ca_path: path_string(&source_ca_cert_path),
                },
            },
            verify: VerifyRuntimeVerifyConfig {
                source: source.defaults_config(),
                destination: destination.defaults_config(),
                databases: vec![VerifyDatabaseMappingConfig {
                    name: "default".to_owned(),
                    source_database: source.database,
                    destination_database: destination.database,
                }],
            },
        };
        let config_path = root_dir.join("verify-service.yml");
        let config_yaml =
            serde_yaml::to_string(&config).expect("verify runtime config should serialize to YAML");
        fs::write(&config_path, config_yaml).expect("verify runtime config should be written");

        Self {
            _temp_dir: temp_dir,
            root_dir,
            listener_port,
        }
    }
}

struct RunningVerifyService {
    process: Child,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
    base_url: String,
    client: Client,
}

impl RunningVerifyService {
    fn start(_run: &VerifyServiceRun, files: &VerifyRuntimeFiles) -> Self {
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
            "verify service runtime did not become ready on {}\n{}",
            self.base_url,
            self.logs(),
        );
    }

    fn start_job(&self, run: &VerifyServiceRun) -> String {
        let response = self
            .client
            .post(format!("{}/jobs", self.base_url))
            .header("content-type", "application/json")
            .body(
                json!({
                    "default_schema_match": run.schema_match,
                    "default_table_match": run.table_match,
                })
                .to_string(),
            )
            .send()
            .unwrap_or_else(|error| panic!("verify service POST /jobs should succeed: {error}"));
        let status = response.status();
        let response_body = response.text().unwrap_or_else(|error| {
            panic!("verify service POST /jobs response should read: {error}")
        });
        assert!(
            status.is_success(),
            "verify service POST /jobs failed with status {}\nresponse body:\n{}\n{}",
            status,
            response_body,
            self.logs(),
        );
        let payload = parse_json::<VerifyStartResponse>(
            response_body.as_str(),
            "verify service start response",
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
                    panic!("verify service GET /jobs/{job_id} should succeed: {error}")
                });
            assert!(
                response.status().is_success(),
                "verify service GET /jobs/{job_id} failed with status {}\n{}",
                response.status(),
                self.logs(),
            );
            let payload = parse_json::<VerifyJobResponse>(
                response
                    .text()
                    .unwrap_or_else(|error| {
                        panic!("verify service job response should read: {error}")
                    })
                    .as_str(),
                "verify service job response",
            );
            if !payload.is_running() {
                return payload;
            }
            thread::sleep(std::time::Duration::from_secs(1));
        }

        panic!(
            "verify service job `{job_id}` did not finish in time\n{}",
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

impl Drop for RunningVerifyService {
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

impl ParsedPostgresUrl {
    fn with_tls(mut self, tls: VerifyDatabaseTlsConfig) -> Self {
        self.tls = self.tls.merged_with(tls);
        self
    }

    fn defaults_config(&self) -> VerifyDatabaseConfig {
        VerifyDatabaseConfig {
            host: self.host.clone(),
            port: self.port,
            username: self.username.clone(),
            password: self.password.clone(),
            sslmode: self.sslmode.clone(),
            tls: if self.tls.is_empty() {
                None
            } else {
                Some(self.tls.clone())
            },
        }
    }
}

impl VerifyDatabaseTlsConfig {
    fn merged_with(self, override_tls: VerifyDatabaseTlsConfig) -> Self {
        Self {
            ca_cert_path: override_tls.ca_cert_path.or(self.ca_cert_path),
            client_cert_path: override_tls.client_cert_path.or(self.client_cert_path),
            client_key_path: override_tls.client_key_path.or(self.client_key_path),
        }
    }

    fn is_empty(&self) -> bool {
        self.ca_cert_path.is_none()
            && self.client_cert_path.is_none()
            && self.client_key_path.is_none()
    }
}

fn parse_postgres_url(raw: &str) -> ParsedPostgresUrl {
    let url = Url::parse(raw).unwrap_or_else(|error| {
        panic!("verify runtime database URL `{raw}` should parse: {error}")
    });
    let scheme = url.scheme();
    assert!(
        matches!(scheme, "postgres" | "postgresql"),
        "verify runtime database URL `{raw}` must use a postgres scheme, got `{scheme}`",
    );
    let host = url
        .host_str()
        .unwrap_or_else(|| panic!("verify runtime database URL `{raw}` must include a host"))
        .to_owned();
    let port = url.port().unwrap_or(5432);
    let username = url.username().to_owned();
    assert!(
        !username.is_empty(),
        "verify runtime database URL `{raw}` must include a username",
    );
    let database = url
        .path()
        .strip_prefix('/')
        .unwrap_or_else(|| {
            panic!("verify runtime database URL `{raw}` must include a database path")
        })
        .to_owned();
    assert!(
        !database.is_empty() && !database.contains('/'),
        "verify runtime database URL `{raw}` must include exactly one database path segment",
    );

    let mut sslmode = None;
    let mut tls = VerifyDatabaseTlsConfig::default();
    for (key, value) in url.query_pairs() {
        match key.as_ref() {
            "sslmode" => {
                assert!(
                    sslmode.is_none(),
                    "verify runtime database URL `{raw}` must not repeat the `sslmode` query parameter",
                );
                sslmode = Some(value.into_owned());
            }
            "sslrootcert" => tls.ca_cert_path = Some(value.into_owned()),
            "sslcert" => tls.client_cert_path = Some(value.into_owned()),
            "sslkey" => tls.client_key_path = Some(value.into_owned()),
            unsupported => panic!(
                "verify runtime database URL `{raw}` uses unsupported query parameter `{unsupported}`",
            ),
        }
    }

    ParsedPostgresUrl {
        host,
        port,
        database,
        username,
        password: url.password().map(ToOwned::to_owned),
        sslmode: sslmode.unwrap_or_else(|| {
            panic!("verify runtime database URL `{raw}` must include an `sslmode` query parameter")
        }),
        tls,
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn parse_json<T>(raw: &str, description: &str) -> T
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(raw)
        .unwrap_or_else(|error| panic!("{description} should parse from JSON `{raw}`: {error}"))
}
