use std::{io, net::AddrParseError, path::PathBuf};

use thiserror::Error;

use crate::schema_compare::SchemaCompareError;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("config: {0}")]
    Config(#[from] RunnerConfigError),
    #[error("postgres setup artifacts: {0}")]
    PostgresSetupArtifacts(#[from] RunnerArtifactError),
    #[error("helper plan: {0}")]
    HelperPlan(#[from] RunnerHelperPlanError),
    #[error("postgres bootstrap: {0}")]
    PostgresBootstrap(#[from] RunnerBootstrapError),
    #[error(transparent)]
    SchemaCompare(#[from] SchemaCompareError),
}

#[derive(Debug, Error)]
pub enum RunnerConfigError {
    #[error("failed to read config file `{path}`")]
    ReadFile { path: PathBuf, source: io::Error },
    #[error("failed to parse config file `{path}`")]
    ParseFile {
        path: PathBuf,
        source: serde_yaml::Error,
    },
    #[error("invalid config field `{field}`: {message}")]
    InvalidField {
        field: &'static str,
        message: &'static str,
    },
    #[error("invalid socket address in `{field}`")]
    InvalidSocketAddr {
        field: &'static str,
        source: AddrParseError,
    },
}

#[derive(Debug, Error)]
pub enum RunnerArtifactError {
    #[error("failed to create output directory `{path}`")]
    CreateOutputDirectory { path: PathBuf, source: io::Error },
    #[error("failed to create mapping directory `{path}`")]
    CreateMappingDirectory { path: PathBuf, source: io::Error },
    #[error("failed to write artifact file `{path}`")]
    WriteFile { path: PathBuf, source: io::Error },
}

#[derive(Debug, Error)]
pub enum RunnerHelperPlanError {
    #[error(transparent)]
    SchemaCompare(#[from] SchemaCompareError),
    #[error(transparent)]
    Artifact(#[from] RunnerArtifactError),
    #[error("validated schema is missing selected table `{table}` for mapping `{mapping_id}`")]
    MissingValidatedTable { mapping_id: String, table: String },
    #[error("dependency cycle detected for mapping `{mapping_id}` across tables: {tables}")]
    DependencyCycle { mapping_id: String, tables: String },
}

#[derive(Debug, Error)]
pub enum RunnerBootstrapError {
    #[error("failed to start async runtime")]
    StartRuntime { source: io::Error },
    #[error("failed to connect mapping `{mapping_id}` to `{endpoint}`: {source}")]
    Connect {
        mapping_id: String,
        endpoint: String,
        source: sqlx::Error,
    },
    #[error("failed to execute bootstrap ddl for mapping `{mapping_id}` in `{database}`: {source}")]
    ExecuteDdl {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
    #[error("failed to read destination table shape for mapping `{mapping_id}` in `{database}` table `{table}`: {source}")]
    ReadCatalog {
        mapping_id: String,
        database: String,
        table: String,
        source: sqlx::Error,
    },
    #[error("failed to build helper plan for mapping `{mapping_id}` in `{database}`: {source}")]
    HelperPlan {
        mapping_id: String,
        database: String,
        source: RunnerHelperPlanError,
    },
    #[error("missing mapped destination table `{table}` for mapping `{mapping_id}` in `{database}`")]
    MissingTable {
        mapping_id: String,
        database: String,
        table: String,
    },
    #[error("unsupported foreign key ON DELETE action `{action}` for mapping `{mapping_id}` in `{database}` table `{table}`")]
    UnsupportedForeignKeyAction {
        mapping_id: String,
        database: String,
        table: String,
        action: String,
    },
    #[error("incomplete foreign key metadata while reading mapping `{mapping_id}` in `{database}` table `{table}`")]
    IncompleteForeignKeyMetadata {
        mapping_id: String,
        database: String,
        table: String,
    },
}
