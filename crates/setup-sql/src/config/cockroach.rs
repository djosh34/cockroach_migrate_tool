use std::path::Path;

use ingest_contract::MappingIngestPath;

use crate::error::BootstrapConfigError;

#[derive(Clone, Debug)]
pub(crate) struct BootstrapConfig {
    pub(super) cockroach_url: String,
    pub(super) webhook: WebhookConfig,
    pub(super) mappings: Vec<SourceMapping>,
}

impl BootstrapConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, BootstrapConfigError> {
        super::cockroach_parser::load(path)
    }

    pub(crate) fn cockroach_url(&self) -> &str {
        &self.cockroach_url
    }

    pub(crate) fn webhook(&self) -> &WebhookConfig {
        &self.webhook
    }

    pub(crate) fn mappings(&self) -> &[SourceMapping] {
        &self.mappings
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WebhookConfig {
    pub(super) base_url: String,
    pub(super) ca_cert_query: String,
    pub(super) resolved: String,
}

impl WebhookConfig {
    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn resolved(&self) -> &str {
        &self.resolved
    }

    pub(crate) fn changefeed_sink_suffix(&self, mapping_id: &str) -> String {
        format!(
            "{}?ca_cert={}",
            MappingIngestPath::new(mapping_id),
            self.ca_cert_query
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SourceMapping {
    pub(super) id: String,
    pub(super) source: SourceSelection,
}

impl SourceMapping {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn source(&self) -> &SourceSelection {
        &self.source
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SourceSelection {
    pub(super) database: String,
    pub(super) tables: Vec<super::TableName>,
}

impl SourceSelection {
    pub(crate) fn database(&self) -> &str {
        &self.database
    }

    pub(crate) fn tables(&self) -> &[super::TableName] {
        &self.tables
    }
}
