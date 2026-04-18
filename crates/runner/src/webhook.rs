use std::net::SocketAddr;

use axum::{Router, routing::get};

use crate::config::WebhookConfig;

pub(crate) struct WebhookRuntime {
    bind_addr: SocketAddr,
    tls_material_label: String,
}

impl WebhookRuntime {
    pub(crate) fn from_config(config: &WebhookConfig) -> Self {
        let _router: Router<()> = Router::new().route("/healthz", get(healthcheck));

        Self {
            bind_addr: config.bind_addr(),
            tls_material_label: config.tls_material_label(),
        }
    }

    pub(crate) fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub(crate) fn tls_material_label(&self) -> &str {
        &self.tls_material_label
    }
}

async fn healthcheck() -> &'static str {
    "ok"
}
