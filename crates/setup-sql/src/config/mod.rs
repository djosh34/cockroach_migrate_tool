mod cockroach_parser;
mod postgres_grants_parser;

use std::path::Path;

use ingest_contract::MappingIngestPath;

use crate::error::BootstrapConfigError;

#[derive(Clone, Debug)]
pub(crate) struct BootstrapConfig {
    cockroach_url: String,
    webhook: WebhookConfig,
    mappings: Vec<SourceMapping>,
}

impl BootstrapConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, BootstrapConfigError> {
        cockroach_parser::load(path)
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
pub(crate) struct PostgresGrantsConfig {
    mappings: Vec<PostgresGrantMapping>,
}

impl PostgresGrantsConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, BootstrapConfigError> {
        postgres_grants_parser::load(path)
    }

    pub(crate) fn mappings(&self) -> &[PostgresGrantMapping] {
        &self.mappings
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PostgresGrantMapping {
    id: String,
    destination: PostgresGrantDestination,
}

impl PostgresGrantMapping {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn destination(&self) -> &PostgresGrantDestination {
        &self.destination
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PostgresGrantDestination {
    database: String,
    runtime_role: String,
    tables: Vec<TableName>,
}

impl PostgresGrantDestination {
    pub(crate) fn database(&self) -> &str {
        &self.database
    }

    pub(crate) fn runtime_role(&self) -> &str {
        &self.runtime_role
    }

    pub(crate) fn tables(&self) -> &[TableName] {
        &self.tables
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WebhookConfig {
    base_url: String,
    ca_cert_query: String,
    resolved: String,
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
    pub(crate) fn new(schema: String, name: String) -> Self {
        Self { schema, name }
    }

    pub(crate) fn display_name(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }

    pub(crate) fn schema(&self) -> &str {
        &self.schema
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn sql_reference_in_database(&self, database: &str) -> String {
        format!("{database}.{}", self.display_name())
    }
}
