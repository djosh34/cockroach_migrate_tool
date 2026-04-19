use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    fs,
    hash::{Hash, Hasher},
    net::TcpListener as StdTcpListener,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use axum::{
    Router,
    body::Bytes,
    extract::{OriginalUri, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder as AutoBuilder,
    service::TowerToHyperService,
};
use reqwest::Certificate;
use rustls::ServerConfig;
use tokio::{net::TcpListener, runtime::Builder, task::JoinSet};
use tokio_rustls::TlsAcceptor;

use crate::e2e_harness::{
    https_client, investigation_ca_cert_path, investigation_server_cert_path,
    investigation_server_key_path,
};

pub(crate) struct WebhookChaosGateway {
    sink_port: u16,
    state: Arc<Mutex<GatewayState>>,
    shutdown_tx: Option<mpsc::Sender<()>>,
    thread_handle: Option<thread::JoinHandle<()>>,
}

struct GatewayAppState {
    upstream_base_url: String,
    upstream_client: reqwest::Client,
    state: Arc<Mutex<GatewayState>>,
}

#[derive(Default)]
struct GatewayState {
    armed_failures: Vec<ArmedFailure>,
    attempts_by_fingerprint: HashMap<String, usize>,
    attempt_log: Vec<ForwardedAttempt>,
}

struct ArmedFailure {
    body_substring: String,
    remaining_failures: usize,
}

struct ForwardedAttempt {
    body: String,
    fingerprint: String,
    attempt_number: usize,
    upstream_status: StatusCode,
    downstream_status: StatusCode,
}

impl WebhookChaosGateway {
    pub(crate) fn start(upstream_runner_port: u16) -> Self {
        let listener =
            StdTcpListener::bind("0.0.0.0:0").expect("webhook chaos gateway listener should bind");
        listener
            .set_nonblocking(true)
            .expect("webhook chaos gateway listener should be nonblocking");
        let sink_port = listener
            .local_addr()
            .expect("webhook chaos gateway listener should have a local address")
            .port();
        let state = Arc::new(Mutex::new(GatewayState::default()));
        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        let thread_state = Arc::clone(&state);
        let thread_handle = thread::spawn(move || {
            let runtime = Builder::new_multi_thread()
                .enable_io()
                .enable_time()
                .build()
                .expect("webhook chaos gateway runtime should build");
            runtime
                .block_on(run_gateway(
                    listener,
                    upstream_runner_port,
                    thread_state,
                    shutdown_rx,
                ))
                .expect("webhook chaos gateway should serve without errors");
        });
        let gateway = Self {
            sink_port,
            state,
            shutdown_tx: Some(shutdown_tx),
            thread_handle: Some(thread_handle),
        };
        gateway.wait_until_healthy();
        gateway
    }

    pub(crate) fn public_base_url(&self) -> String {
        format!("https://host.docker.internal:{}", self.sink_port)
    }

    pub(crate) fn arm_single_external_http_500_for_body_substring(&self, body_substring: &str) {
        let mut state = self
            .state
            .lock()
            .expect("webhook chaos gateway state should lock");
        state.armed_failures.push(ArmedFailure {
            body_substring: body_substring.to_owned(),
            remaining_failures: 1,
        });
    }

    pub(crate) fn has_duplicate_delivery_for_body_substring(&self, body_substring: &str) -> bool {
        let state = self
            .state
            .lock()
            .expect("webhook chaos gateway state should lock");
        state
            .attempt_log
            .iter()
            .filter(|attempt| attempt.body.contains(body_substring))
            .any(|attempt| {
                state
                    .attempts_by_fingerprint
                    .get(&attempt.fingerprint)
                    .copied()
                    .unwrap_or_default()
                    >= 2
            })
    }

    pub(crate) fn attempt_summary_for_body_substring(&self, body_substring: &str) -> String {
        let state = self
            .state
            .lock()
            .expect("webhook chaos gateway state should lock");
        state
            .attempt_log
            .iter()
            .filter(|attempt| attempt.body.contains(body_substring))
            .map(|attempt| {
                format!(
                    "fingerprint={} attempt={} upstream={} downstream={}",
                    attempt.fingerprint,
                    attempt.attempt_number,
                    attempt.upstream_status,
                    attempt.downstream_status,
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn wait_until_healthy(&self) {
        let client = https_client(&investigation_ca_cert_path());
        for _ in 0..60 {
            match client
                .get(format!("https://localhost:{}/healthz", self.sink_port))
                .send()
            {
                Ok(response) if response.status().is_success() => return,
                Ok(_) | Err(_) => thread::sleep(Duration::from_millis(100)),
            }
        }

        panic!(
            "webhook chaos gateway did not become healthy on https://localhost:{}/healthz",
            self.sink_port
        );
    }
}

impl Drop for WebhookChaosGateway {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            shutdown_tx
                .send(())
                .expect("webhook chaos gateway shutdown signal should send");
        }
        if let Some(thread_handle) = self.thread_handle.take() {
            thread_handle
                .join()
                .expect("webhook chaos gateway thread should join");
        }
    }
}

async fn run_gateway(
    listener: StdTcpListener,
    upstream_runner_port: u16,
    state: Arc<Mutex<GatewayState>>,
    shutdown_rx: mpsc::Receiver<()>,
) -> Result<(), String> {
    let listener = TcpListener::from_std(listener).map_err(|error| {
        format!("webhook chaos gateway listener should convert to tokio: {error}")
    })?;
    let tls_acceptor = TlsAcceptor::from(Arc::new(load_tls_config()?));
    let certificate = Certificate::from_pem(
        &fs::read(investigation_ca_cert_path())
            .expect("webhook chaos gateway CA certificate should be readable"),
    )
    .expect("webhook chaos gateway CA certificate should parse");
    let upstream_client = reqwest::Client::builder()
        .add_root_certificate(certificate)
        .build()
        .expect("webhook chaos gateway client should build");
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/", any(proxy_request))
        .route("/{*path}", any(proxy_request))
        .with_state(Arc::new(GatewayAppState {
            upstream_base_url: format!("https://localhost:{upstream_runner_port}"),
            upstream_client,
            state,
        }));
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                let (tcp_stream, _) = accept_result
                    .map_err(|error| format!("webhook chaos gateway accept failed: {error}"))?;
                let tls_acceptor = tls_acceptor.clone();
                let service = app.clone();
                connections.spawn(async move {
                    let tls_stream = tls_acceptor
                        .accept(tcp_stream)
                        .await
                        .map_err(|error| format!("webhook chaos gateway TLS handshake failed: {error}"))?;
                    let io = TokioIo::new(tls_stream);
                    AutoBuilder::new(TokioExecutor::new())
                        .serve_connection_with_upgrades(io, TowerToHyperService::new(service))
                        .await
                        .map_err(|error| format!("webhook chaos gateway connection failed: {error}"))
                });
            }
            Some(connection_result) = connections.join_next(), if !connections.is_empty() => {
                match connection_result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => return Err(error),
                    Err(error) => return Err(format!("webhook chaos gateway connection task failed: {error}")),
                }
            }
            _ = tokio::time::sleep(Duration::from_millis(100)) => {
                if shutdown_rx.try_recv().is_ok() {
                    return Ok(());
                }
            }
        }
    }
}

async fn healthz() -> &'static str {
    "ok"
}

async fn proxy_request(
    State(app): State<Arc<GatewayAppState>>,
    method: Method,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    body: Bytes,
) -> Response {
    let forward_url = format!("{}{}", app.upstream_base_url, uri);
    let body_text = String::from_utf8_lossy(body.as_ref()).into_owned();
    let fingerprint = request_fingerprint(&uri.to_string(), &body_text);
    let override_status = {
        let mut state = app
            .state
            .lock()
            .expect("webhook chaos gateway state should lock");
        state
            .armed_failures
            .iter_mut()
            .find(|rule| {
                rule.remaining_failures > 0 && body_text.contains(rule.body_substring.as_str())
            })
            .map(|rule| {
                rule.remaining_failures -= 1;
                StatusCode::INTERNAL_SERVER_ERROR
            })
    };

    let mut request = app.upstream_client.request(method, forward_url).body(body);
    if let Some(content_type) = headers.get(header::CONTENT_TYPE) {
        request = request.header(header::CONTENT_TYPE, content_type);
    }
    let upstream_response = request
        .send()
        .await
        .expect("webhook chaos gateway should forward the request to the runner");
    let upstream_status = upstream_response.status();
    let downstream_status = override_status.unwrap_or(upstream_status);
    let upstream_headers = upstream_response.headers().clone();
    let upstream_body = upstream_response
        .bytes()
        .await
        .expect("webhook chaos gateway should read the upstream response body");

    {
        let mut state = app
            .state
            .lock()
            .expect("webhook chaos gateway state should lock");
        let attempt_number = {
            let entry = state
                .attempts_by_fingerprint
                .entry(fingerprint.clone())
                .or_insert(0);
            *entry += 1;
            *entry
        };
        state.attempt_log.push(ForwardedAttempt {
            body: body_text,
            fingerprint,
            attempt_number,
            upstream_status,
            downstream_status,
        });
    }

    response_with_optional_content_type(
        downstream_status,
        upstream_headers.get(header::CONTENT_TYPE),
        upstream_body,
    )
}

fn response_with_optional_content_type(
    status: StatusCode,
    content_type: Option<&HeaderValue>,
    body: Bytes,
) -> Response {
    let mut response = (status, body).into_response();
    if let Some(content_type) = content_type {
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, content_type.clone());
    }
    response
}

fn request_fingerprint(path: &str, body: &str) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    body.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn load_tls_config() -> Result<ServerConfig, String> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|_| {
                "webhook chaos gateway should install the rustls crypto provider".to_owned()
            })?;
    }

    let cert_contents = fs::read(investigation_server_cert_path()).map_err(|error| {
        format!("webhook chaos gateway should read the TLS certificate: {error}")
    })?;
    let key_contents = fs::read(investigation_server_key_path()).map_err(|error| {
        format!("webhook chaos gateway should read the TLS private key: {error}")
    })?;
    let certificates = rustls_pemfile::certs(&mut cert_contents.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!("webhook chaos gateway should parse the TLS certificate: {error}")
        })?;
    if certificates.is_empty() {
        return Err("webhook chaos gateway should have at least one TLS certificate".to_owned());
    }

    let private_key =
        rustls_pemfile::private_key(&mut key_contents.as_slice()).map_err(|error| {
            format!("webhook chaos gateway should parse the TLS private key: {error}")
        })?;
    let Some(private_key) = private_key else {
        return Err("webhook chaos gateway should have a TLS private key".to_owned());
    };

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificates, private_key)
        .map_err(|error| format!("webhook chaos gateway should build the TLS config: {error}"))
}
