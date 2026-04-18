use std::{net::SocketAddr, path::PathBuf};

use axum::{Router, routing::get};

use crate::config::WebhookConfig;

pub(crate) struct WebhookRuntime {
    bind_addr: SocketAddr,
    tls_cert_path: PathBuf,
    tls_key_path: PathBuf,
}

impl WebhookRuntime {
    pub(crate) fn from_config(config: &WebhookConfig) -> Self {
        let _router: Router<()> = Router::new().route("/healthz", get(healthcheck));

        Self {
            bind_addr: config.bind_addr(),
            tls_cert_path: config.tls_cert_path().to_path_buf(),
            tls_key_path: config.tls_key_path().to_path_buf(),
        }
    }

    pub(crate) fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub(crate) fn tls_material_label(&self) -> String {
        format!(
            "{}+{}",
            self.tls_cert_path.display(),
            self.tls_key_path.display()
        )
    }
}

async fn healthcheck() -> &'static str {
    "ok"
}
