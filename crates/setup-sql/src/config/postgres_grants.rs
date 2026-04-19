use std::path::Path;

use crate::error::BootstrapConfigError;

#[derive(Clone, Debug)]
pub(crate) struct PostgresGrantsConfig {
    pub(super) mappings: Vec<PostgresGrantMapping>,
}

impl PostgresGrantsConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, BootstrapConfigError> {
        super::postgres_grants_parser::load(path)
    }

    pub(crate) fn mappings(&self) -> &[PostgresGrantMapping] {
        &self.mappings
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PostgresGrantMapping {
    pub(super) destination: PostgresGrantDestination,
}

impl PostgresGrantMapping {
    pub(crate) fn destination(&self) -> &PostgresGrantDestination {
        &self.destination
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PostgresGrantDestination {
    pub(super) database: String,
    pub(super) runtime_role: String,
    pub(super) tables: Vec<super::TableName>,
}

impl PostgresGrantDestination {
    pub(crate) fn database(&self) -> &str {
        &self.database
    }

    pub(crate) fn runtime_role(&self) -> &str {
        &self.runtime_role
    }

    pub(crate) fn tables(&self) -> &[super::TableName] {
        &self.tables
    }
}
