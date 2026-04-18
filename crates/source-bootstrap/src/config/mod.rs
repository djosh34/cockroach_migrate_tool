mod parser;

use std::path::Path;

use crate::error::BootstrapConfigError;

#[derive(Clone, Debug)]
pub(crate) struct BootstrapConfig {
    cockroach_url: String,
    webhook: WebhookConfig,
    mappings: Vec<SourceMapping>,
}

impl BootstrapConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, BootstrapConfigError> {
        parser::load(path)
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
    base_url: String,
    resolved: String,
}

impl WebhookConfig {
    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn resolved(&self) -> &str {
        &self.resolved
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SourceMapping {
    id: String,
    source: SourceSelection,
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
    database: String,
    tables: Vec<TableName>,
}

impl SourceSelection {
    pub(crate) fn database(&self) -> &str {
        &self.database
    }

    pub(crate) fn tables(&self) -> &[TableName] {
        &self.tables
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TableName {
    schema: String,
    name: String,
}

impl TableName {
    pub(crate) fn display_name(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }

    pub(crate) fn sql_reference(&self) -> String {
        self.display_name()
    }
}
