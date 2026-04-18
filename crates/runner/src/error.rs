use std::{io, net::AddrParseError, path::PathBuf};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("config: {0}")]
    Config(#[from] RunnerConfigError),
    #[error("postgres setup artifacts: {0}")]
    PostgresSetupArtifacts(#[from] RunnerArtifactError),
    #[error("postgres bootstrap: {0}")]
    PostgresBootstrap(#[from] RunnerBootstrapError),
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
    #[error("missing mapped destination table `{table}` for mapping `{mapping_id}` in `{database}`")]
    MissingTable {
        mapping_id: String,
        database: String,
        table: String,
    },
}
