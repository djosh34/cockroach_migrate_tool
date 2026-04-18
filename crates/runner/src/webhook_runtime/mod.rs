mod persistence;
mod routing;
mod payload;

use std::{fs, sync::Arc};

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
use rustls::ServerConfig;
use tokio::{net::TcpListener, task::JoinSet};
use tokio_rustls::TlsAcceptor;

use crate::{
    error::{
        RunnerIngressRequestError, RunnerWebhookRoutingError, RunnerWebhookRuntimeError,
    },
    runtime_plan::RunnerRuntimePlan,
    tracking_state::persist_resolved_watermark,
};
use persistence::persist_row_batch;
use payload::parse_webhook_request;

pub(crate) async fn serve(
    runtime: Arc<RunnerRuntimePlan>,
) -> Result<(), RunnerWebhookRuntimeError> {
    let listener = TcpListener::bind(runtime.bind_addr())
        .await
        .map_err(|source| RunnerWebhookRuntimeError::Bind {
            addr: runtime.bind_addr(),
            source,
        })?;
    let tls_acceptor = TlsAcceptor::from(Arc::new(load_tls_config(runtime.as_ref())?));
    let app = Router::new()
        .route("/healthz", get(healthz))
        .route("/ingest/{mapping_id}", post(ingest))
        .with_state(runtime.clone());
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
                        .map_err(|source| RunnerWebhookRuntimeError::ServeConnection {
                            source,
                        })
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

    let cert_contents = fs::read(runtime.tls_cert_path()).map_err(|source| {
        RunnerWebhookRuntimeError::ReadTlsCertificate {
            path: runtime.tls_cert_path().to_path_buf(),
            source,
        }
    })?;
    let key_contents = fs::read(runtime.tls_key_path()).map_err(|source| {
        RunnerWebhookRuntimeError::ReadTlsPrivateKey {
            path: runtime.tls_key_path().to_path_buf(),
            source,
        }
    })?;
    let certificates = rustls_pemfile::certs(&mut cert_contents.as_slice())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| RunnerWebhookRuntimeError::ReadTlsCertificate {
            path: runtime.tls_cert_path().to_path_buf(),
            source,
        })?;
    if certificates.is_empty() {
        return Err(RunnerWebhookRuntimeError::MissingTlsCertificate {
            path: runtime.tls_cert_path().to_path_buf(),
        });
    }

    let private_key = rustls_pemfile::private_key(&mut key_contents.as_slice()).map_err(|source| {
        RunnerWebhookRuntimeError::ReadTlsPrivateKey {
            path: runtime.tls_key_path().to_path_buf(),
            source,
        }
    })?;
    let Some(private_key) = private_key else {
        return Err(RunnerWebhookRuntimeError::MissingTlsPrivateKey {
            path: runtime.tls_key_path().to_path_buf(),
        });
    };

    ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certificates, private_key)
        .map_err(|source| RunnerWebhookRuntimeError::BuildTlsConfig { source })
}

async fn healthz() -> &'static str {
    "ok"
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
    let dispatch_target = routing::route_request(mapping, request)?;
    dispatch(dispatch_target).await
}

async fn dispatch(
    dispatch_target: routing::DispatchTarget,
) -> Result<StatusCode, RunnerIngressRequestError> {
    match dispatch_target {
        routing::DispatchTarget::RowBatch(batch) => {
            persist_row_batch(*batch).await?;
            Ok(StatusCode::OK)
        }
        routing::DispatchTarget::Resolved(target) => {
            persist_resolved_watermark(target).await?;
            Ok(StatusCode::OK)
        }
    }
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
