mod payload;
mod persistence;
mod routing;

use std::{
    fs,
    sync::Arc,
    time::{Instant, SystemTime},
};

use axum::{
    Router,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder as AutoBuilder,
    service::TowerToHyperService,
};
use operator_log::LogEvent;
use rustls::ServerConfig;
use tokio::{net::TcpListener, task::JoinSet};
use tokio_rustls::TlsAcceptor;

use crate::{
    RuntimeEventSink,
    error::{RunnerIngressRequestError, RunnerWebhookRoutingError, RunnerWebhookRuntimeError},
    metrics::WebhookOutcome,
    runtime_plan::{RunnerRuntimePlan, WebhookListenerTransport},
    tracking_state::{ResolvedTrackingTarget, persist_resolved_watermark},
};
use payload::parse_webhook_request;
use persistence::{RowMutationBatch, persist_row_batch};

pub(crate) async fn serve(
    runtime: Arc<RunnerRuntimePlan>,
    emit_event: RuntimeEventSink,
) -> Result<(), RunnerWebhookRuntimeError> {
    let listener = TcpListener::bind(runtime.bind_addr())
        .await
        .map_err(|source| RunnerWebhookRuntimeError::Bind {
            addr: runtime.bind_addr(),
            source,
        })?;
    let bound_addr = listener
        .local_addr()
        .map_err(|source| RunnerWebhookRuntimeError::LocalAddr { source })?;
    emit_event(
        LogEvent::info("runner", "webhook.bound", "runner webhook listener bound")
            .with_field("webhook", bound_addr.to_string())
            .with_field(
                "mode",
                match runtime.webhook_listener().transport() {
                    WebhookListenerTransport::Http => "http",
                    WebhookListenerTransport::Https { .. } => "https",
                },
            ),
    );
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/metrics", get(metrics))
        .route("/ingest/{mapping_id}", post(ingest))
        .with_state(runtime.clone());
    match runtime.webhook_listener().transport() {
        WebhookListenerTransport::Http => axum::serve(listener, app)
            .await
            .map_err(|source| RunnerWebhookRuntimeError::ServeConnection {
                source: Box::new(source),
            }),
        WebhookListenerTransport::Https { .. } => {
            let tls_acceptor = TlsAcceptor::from(Arc::new(load_tls_config(runtime.as_ref())?));
            serve_https(listener, app, tls_acceptor).await
        }
    }
}

async fn serve_https(
    listener: TcpListener,
    app: Router,
    tls_acceptor: TlsAcceptor,
) -> Result<(), RunnerWebhookRuntimeError> {
    let mut connections = JoinSet::new();

    loop {
        tokio::select! {
            accept_result = listener.accept() => {
                let (tcp_stream, _) = accept_result
                    .map_err(|source| RunnerWebhookRuntimeError::Accept { source })?;
                let tls_acceptor = tls_acceptor.clone();
                let service = app.clone();
                connections.spawn(async move {
                    let tls_stream = tls_acceptor
                        .accept(tcp_stream)
                        .await
                        .map_err(|source| RunnerWebhookRuntimeError::TlsHandshake { source })?;
                    let io = TokioIo::new(tls_stream);
                    AutoBuilder::new(TokioExecutor::new())
                        .serve_connection_with_upgrades(io, TowerToHyperService::new(service))
                        .await
                        .map_err(|source| RunnerWebhookRuntimeError::ServeConnection { source })
                });
            }
            Some(result) = connections.join_next(), if !connections.is_empty() => {
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => return Err(error),
                    Err(source) => return Err(RunnerWebhookRuntimeError::ConnectionTask { source }),
                }
            }
        }
    }
}

fn load_tls_config(runtime: &RunnerRuntimePlan) -> Result<ServerConfig, RunnerWebhookRuntimeError> {
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|_| RunnerWebhookRuntimeError::InstallCryptoProvider)?;
    }

    let tls = runtime
        .webhook_listener()
        .transport()
        .tls()
        .unwrap_or_else(|| panic!("https listener must expose tls material"));
    let cert_contents = fs::read(tls.cert_path()).map_err(|source| {
        RunnerWebhookRuntimeError::ReadTlsCertificate {
            path: tls.cert_path().to_path_buf(),
            source,
        }
    })?;
    let key_contents = fs::read(tls.key_path()).map_err(|source| {
        RunnerWebhookRuntimeError::ReadTlsPrivateKey {
            path: tls.key_path().to_path_buf(),
            source,
        }
    })?;
    let certificates = rustls_pemfile::certs(&mut cert_contents.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| RunnerWebhookRuntimeError::ReadTlsCertificate {
            path: tls.cert_path().to_path_buf(),
            source,
        })?;
    if certificates.is_empty() {
        return Err(RunnerWebhookRuntimeError::MissingTlsCertificate {
            path: tls.cert_path().to_path_buf(),
        });
    }

    let private_key =
        rustls_pemfile::private_key(&mut key_contents.as_slice()).map_err(|source| {
            RunnerWebhookRuntimeError::ReadTlsPrivateKey {
                path: tls.key_path().to_path_buf(),
                source,
            }
        })?;
    let Some(private_key) = private_key else {
        return Err(RunnerWebhookRuntimeError::MissingTlsPrivateKey {
            path: tls.key_path().to_path_buf(),
        });
    };

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificates, private_key)
        .map_err(|source| RunnerWebhookRuntimeError::BuildTlsConfig { source })?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(config)
}

async fn healthz() -> &'static str {
    "ok"
}

async fn metrics(State(runtime): State<Arc<RunnerRuntimePlan>>) -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        runtime.metrics().render(),
    )
}

async fn ingest(
    Path(mapping_id): Path<String>,
    State(runtime): State<Arc<RunnerRuntimePlan>>,
    body: Bytes,
) -> impl IntoResponse {
    match handle_ingest(mapping_id, runtime, body).await {
        Ok(status) => status.into_response(),
        Err(error) => error.into_response(),
    }
}

async fn handle_ingest(
    mapping_id: String,
    runtime: Arc<RunnerRuntimePlan>,
    body: Bytes,
) -> Result<StatusCode, RunnerIngressRequestError> {
    let mapping = runtime.require_mapping(&mapping_id)?;
    let request = parse_webhook_request(body.as_ref())?;
    let kind = request.kind();
    let result = match routing::route_request(mapping, request) {
        Ok(routing::DispatchTarget::RowBatch(batch)) => {
            handle_row_batch(runtime.as_ref(), mapping, *batch).await
        }
        Ok(routing::DispatchTarget::Resolved(target)) => handle_resolved(*target).await,
        Err(error) => Err(error.into()),
    };
    let outcome = match &result {
        Ok(_) => WebhookOutcome::Ok,
        Err(RunnerIngressRequestError::Persistence(_)) => WebhookOutcome::InternalError,
        Err(RunnerIngressRequestError::Payload(_) | RunnerIngressRequestError::Routing(_)) => {
            WebhookOutcome::BadRequest
        }
    };
    runtime
        .metrics()
        .record_webhook_request(mapping, kind, outcome, SystemTime::now());
    result
}

async fn handle_row_batch(
    runtime: &RunnerRuntimePlan,
    mapping: &crate::runtime_plan::MappingRuntimePlan,
    batch: RowMutationBatch,
) -> Result<StatusCode, RunnerIngressRequestError> {
    let table = batch.table.clone();
    let started_at = Instant::now();
    match persist_row_batch(batch).await {
        Ok(()) => {
            runtime.metrics().record_webhook_apply(
                mapping,
                &table,
                started_at.elapsed(),
                SystemTime::now(),
            );
            Ok(StatusCode::OK)
        }
        Err(error) => {
            runtime
                .metrics()
                .record_webhook_apply_failure(mapping, &table, SystemTime::now());
            Err(error.into())
        }
    }
}

async fn handle_resolved(
    target: ResolvedTrackingTarget,
) -> Result<StatusCode, RunnerIngressRequestError> {
    persist_resolved_watermark(target).await?;
    Ok(StatusCode::OK)
}

impl IntoResponse for RunnerIngressRequestError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::Routing(RunnerWebhookRoutingError::UnknownMapping { .. }) => {
                StatusCode::NOT_FOUND
            }
            Self::Payload(_) | Self::Routing(_) => StatusCode::BAD_REQUEST,
            Self::Persistence(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        match self {
            Self::Routing(_) | Self::Payload(_) | Self::Persistence(_) => {
                (status, self.to_string()).into_response()
            }
        }
    }
}
