use std::{io, net::AddrParseError, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerConfigError {
    #[error("failed to read config file `{path}`")]
    ReadFile { path: PathBuf, source: io::Error },
    #[error("failed to parse config file `{path}`: {source}")]
    ParseFile {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    #[error("invalid config field `{field}`: {message}")]
    InvalidField {
        field: &'static str,
        message: &'static str,
    },
    #[error("invalid config field `{field}`: {message}")]
    InvalidFieldDetail {
        field: &'static str,
        message: String,
    },
    #[error("invalid socket address in `{field}`")]
    InvalidSocketAddr {
        field: &'static str,
        source: AddrParseError,
    },
}

#[derive(Debug, Error)]
pub enum RunnerDestinationCatalogError {
    #[error("failed to connect mapping `{mapping_id}` to `{endpoint}`: {source}")]
    Connect {
        mapping_id: String,
        endpoint: String,
        source: sqlx::Error,
    },
    #[error(
        "failed to read destination table shape for mapping `{mapping_id}` in `{database}` table `{table}`: {source}"
    )]
    ReadCatalog {
        mapping_id: String,
        database: String,
        table: String,
        source: sqlx::Error,
    },
    #[error(
        "missing mapped destination table `{table}` for mapping `{mapping_id}` in `{database}`"
    )]
    MissingTable {
        mapping_id: String,
        database: String,
        table: String,
    },
    #[error(
        "unsupported foreign key ON DELETE action `{action}` for mapping `{mapping_id}` in `{database}` table `{table}`"
    )]
    UnsupportedForeignKeyAction {
        mapping_id: String,
        database: String,
        table: String,
        action: String,
    },
    #[error(
        "incomplete foreign key metadata while reading mapping `{mapping_id}` in `{database}` table `{table}`"
    )]
    IncompleteForeignKeyMetadata {
        mapping_id: String,
        database: String,
        table: String,
    },
}

#[derive(Debug, Error)]
pub enum RunnerStartupPlanError {
    #[error(
        "destination database `{destination}` has conflicting PostgreSQL target contracts for mappings `{first_mapping_id}` and `{second_mapping_id}`"
    )]
    InconsistentDestinationTarget {
        destination: String,
        first_mapping_id: String,
        second_mapping_id: String,
    },
    #[error(
        "destination database `{destination}` table `{table}` is claimed by both mappings `{first_mapping_id}` and `{second_mapping_id}`"
    )]
    OverlappingDestinationTable {
        destination: String,
        table: String,
        first_mapping_id: String,
        second_mapping_id: String,
    },
}

#[derive(Debug, Error)]
pub enum RunnerValidateConfigError {
    #[error(transparent)]
    Config(#[from] RunnerConfigError),
    #[error(transparent)]
    StartupPlan(#[from] RunnerStartupPlanError),
    #[error(transparent)]
    DestinationCatalog(#[from] RunnerDestinationCatalogError),
}
