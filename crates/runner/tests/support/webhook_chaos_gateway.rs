use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    error::Error,
    fmt, fs,
    hash::{Hash, Hasher},
    net::TcpListener as StdTcpListener,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

use axum::{
    body::Bytes,
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
    response::{IntoResponse, Response},
};
use http_body_util::BodyExt;
use hyper::{body::Incoming, service::service_fn};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder as AutoBuilder,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExternalSinkFault {
    HttpStatus { status: StatusCode },
    AbortConnectionBeforeForward,
}

impl fmt::Display for ExternalSinkFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HttpStatus { status } => write!(f, "http_status={status}"),
            Self::AbortConnectionBeforeForward => f.write_str("abort_connection_before_forward"),
        }
    }
}

struct GatewayAppState {
    upstream_base_url: String,
    upstream_client: reqwest::Client,
    state: Arc<Mutex<GatewayState>>,
}

#[derive(Default)]
struct GatewayState {
    armed_faults: Vec<ArmedFault>,
    attempts_by_fingerprint: HashMap<String, usize>,
    attempt_log: Vec<GatewayAttempt>,
}

struct ArmedFault {
    body_substring: String,
    remaining_activations: usize,
    fault: ExternalSinkFault,
}

struct GatewayAttempt {
    body: String,
    fingerprint: String,
    attempt_number: usize,
    outcome: GatewayAttemptOutcome,
}

struct GatewayRequestContext {
    method: Method,
    headers: HeaderMap,
    uri: String,
    body: Bytes,
    body_text: String,
    fingerprint: String,
}

enum GatewayAttemptOutcome {
    InjectedFault(ExternalSinkFault),
    Forwarded {
        upstream_status: StatusCode,
        downstream_status: StatusCode,
    },
}

#[derive(Debug)]
struct InjectedConnectionAbort;

impl fmt::Display for InjectedConnectionAbort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("gateway injected a connection abort before forwarding")
    }
}

impl Error for InjectedConnectionAbort {}

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
        format!("https://127.0.0.1:{}", self.sink_port)
    }

    pub(crate) fn arm_single_external_fault_for_body_substring(
        &self,
        body_substring: &str,
        fault: ExternalSinkFault,
    ) {
        let mut state = self
            .state
            .lock()
            .expect("webhook chaos gateway state should lock");
        state.armed_faults.push(ArmedFault {
            body_substring: body_substring.to_owned(),
            remaining_activations: 1,
            fault,
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

    pub(crate) fn has_fault_then_forward_success_for_body_substring(
        &self,
        body_substring: &str,
        fault: ExternalSinkFault,
    ) -> bool {
        let state = self
            .state
            .lock()
            .expect("webhook chaos gateway state should lock");
        let mut outcomes_by_fingerprint: HashMap<&str, (bool, bool)> = HashMap::new();
        for attempt in state
            .attempt_log
            .iter()
            .filter(|attempt| attempt.body.contains(body_substring))
        {
            let entry = outcomes_by_fingerprint
                .entry(attempt.fingerprint.as_str())
                .or_insert((false, false));
            match attempt.outcome {
                GatewayAttemptOutcome::InjectedFault(recorded_fault) if recorded_fault == fault => {
                    entry.0 = true;
                }
                GatewayAttemptOutcome::Forwarded {
                    downstream_status, ..
                } if downstream_status.is_success() => {
                    entry.1 = true;
                }
                GatewayAttemptOutcome::InjectedFault(_)
                | GatewayAttemptOutcome::Forwarded { .. } => {}
            }
        }
        outcomes_by_fingerprint
            .values()
            .any(|(fault_seen, success_seen)| *fault_seen && *success_seen)
    }

    pub(crate) fn has_forwarded_downstream_status_sequence_for_body_substring(
        &self,
        body_substring: &str,
        statuses: &[StatusCode],
    ) -> bool {
        if statuses.is_empty() {
            return true;
        }

        let state = self
            .state
            .lock()
            .expect("webhook chaos gateway state should lock");
        let observed = state
            .attempt_log
            .iter()
            .filter(|attempt| attempt.body.contains(body_substring))
            .filter_map(|attempt| match attempt.outcome {
                GatewayAttemptOutcome::Forwarded {
                    downstream_status, ..
                } => Some(downstream_status),
                GatewayAttemptOutcome::InjectedFault(_) => None,
            })
            .collect::<Vec<_>>();
        contains_status_subsequence(&observed, statuses)
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
                let outcome = match attempt.outcome {
                    GatewayAttemptOutcome::InjectedFault(fault) => format!("fault={fault}"),
                    GatewayAttemptOutcome::Forwarded {
                        upstream_status,
                        downstream_status,
                    } => format!(
                        "upstream={} downstream={}",
                        upstream_status, downstream_status
                    ),
                };
                format!(
                    "fingerprint={} attempt={} {}",
                    attempt.fingerprint, attempt.attempt_number, outcome,
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub(crate) fn attempt_count_for_body_substring(&self, body_substring: &str) -> usize {
        let state = self
            .state
            .lock()
            .expect("webhook chaos gateway state should lock");
        state
            .attempt_log
            .iter()
            .filter(|attempt| attempt.body.contains(body_substring))
            .count()
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
    let app = Arc::new(GatewayAppState {
        upstream_base_url: format!("https://localhost:{upstream_runner_port}"),
        upstream_client,
        state,
    });
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                let (tcp_stream, _) = accept_result
                    .map_err(|error| format!("webhook chaos gateway accept failed: {error}"))?;
                let tls_acceptor = tls_acceptor.clone();
                let app = Arc::clone(&app);
                connections.spawn(async move {
                    let tls_stream = tls_acceptor
                        .accept(tcp_stream)
                        .await
                        .map_err(|error| format!("webhook chaos gateway TLS handshake failed: {error}"))?;
                    let io = TokioIo::new(tls_stream);
                    let service = service_fn(move |request: Request<Incoming>| {
                        let app = Arc::clone(&app);
                        async move { handle_request(app, request).await }
                    });
                    match AutoBuilder::new(TokioExecutor::new())
                        .serve_connection_with_upgrades(io, service)
                        .await
                    {
                        Ok(()) => Ok(()),
                        Err(error) if is_injected_abort_error(error.as_ref()) => Ok(()),
                        Err(error) => Err(format!("webhook chaos gateway connection failed: {error}")),
                    }
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

async fn handle_request(
    app: Arc<GatewayAppState>,
    request: Request<Incoming>,
) -> Result<Response, InjectedConnectionAbort> {
    let uri = request
        .uri()
        .path_and_query()
        .map(|path_and_query| path_and_query.as_str().to_owned())
        .unwrap_or_else(|| request.uri().path().to_owned());
    if uri == "/healthz" {
        return Ok("ok".into_response());
    }

    let method = request.method().clone();
    let headers = request.headers().clone();
    let body = request
        .into_body()
        .collect()
        .await
        .map(|collected| collected.to_bytes())
        .expect("webhook chaos gateway should read the incoming request body");
    let body_text = String::from_utf8_lossy(body.as_ref()).into_owned();
    let fingerprint = request_fingerprint(&uri, &body_text);
    let request = GatewayRequestContext {
        method,
        headers,
        uri,
        body,
        body_text,
        fingerprint,
    };
    if let Some(fault) = consume_matching_fault(&app.state, &request.body_text) {
        if fault == ExternalSinkFault::AbortConnectionBeforeForward {
            record_attempt(
                &app.state,
                request.body_text,
                request.fingerprint,
                GatewayAttemptOutcome::InjectedFault(fault),
            );
            return Err(InjectedConnectionAbort);
        }
        return Ok(forward_request(app, request, Some(fault)).await);
    }

    Ok(forward_request(app, request, None).await)
}

async fn forward_request(
    app: Arc<GatewayAppState>,
    request: GatewayRequestContext,
    override_fault: Option<ExternalSinkFault>,
) -> Response {
    let forward_url = format!("{}{}", app.upstream_base_url, request.uri);
    let mut upstream_request = app
        .upstream_client
        .request(request.method, forward_url)
        .body(request.body);
    if let Some(content_type) = request.headers.get(header::CONTENT_TYPE) {
        upstream_request = upstream_request.header(header::CONTENT_TYPE, content_type);
    }
    let upstream_response = upstream_request
        .send()
        .await
        .expect("webhook chaos gateway should forward the request to the runner");
    let upstream_status = upstream_response.status();
    let downstream_status = match override_fault {
        Some(ExternalSinkFault::HttpStatus { status }) => status,
        Some(ExternalSinkFault::AbortConnectionBeforeForward) => {
            panic!("abort fault should have returned before forwarding")
        }
        None => upstream_status,
    };
    let upstream_headers = upstream_response.headers().clone();
    let upstream_body = upstream_response
        .bytes()
        .await
        .expect("webhook chaos gateway should read the upstream response body");

    record_attempt(
        &app.state,
        request.body_text,
        request.fingerprint,
        GatewayAttemptOutcome::Forwarded {
            upstream_status,
            downstream_status,
        },
    );

    response_with_optional_content_type(
        downstream_status,
        upstream_headers.get(header::CONTENT_TYPE),
        upstream_body,
    )
}

fn consume_matching_fault(
    state: &Arc<Mutex<GatewayState>>,
    body_text: &str,
) -> Option<ExternalSinkFault> {
    let mut state = state
        .lock()
        .expect("webhook chaos gateway state should lock");
    state
        .armed_faults
        .iter_mut()
        .find(|rule| {
            rule.remaining_activations > 0 && body_text.contains(rule.body_substring.as_str())
        })
        .map(|rule| {
            rule.remaining_activations -= 1;
            rule.fault
        })
}

fn record_attempt(
    state: &Arc<Mutex<GatewayState>>,
    body: String,
    fingerprint: String,
    outcome: GatewayAttemptOutcome,
) {
    let mut state = state
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
    state.attempt_log.push(GatewayAttempt {
        body,
        fingerprint,
        attempt_number,
        outcome,
    });
}

fn is_injected_abort_error(mut error: &(dyn Error + 'static)) -> bool {
    loop {
        if error.downcast_ref::<InjectedConnectionAbort>().is_some() {
            return true;
        }
        if error
            .to_string()
            .contains("gateway injected a connection abort before forwarding")
        {
            return true;
        }
        let Some(source) = error.source() else {
            return false;
        };
        error = source;
    }
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

fn contains_status_subsequence(observed: &[StatusCode], expected: &[StatusCode]) -> bool {
    let mut next_expected = 0usize;
    for status in observed {
        if *status == expected[next_expected] {
            next_expected += 1;
            if next_expected == expected.len() {
                return true;
            }
        }
    }
    false
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
