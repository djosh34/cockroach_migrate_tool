use std::{io, net::AddrParseError, path::PathBuf};

use thiserror::Error;
use tokio::task::JoinError;

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
    #[error("webhook runtime: {0}")]
    WebhookRuntime(#[from] RunnerWebhookRuntimeError),
    #[error("webhook request: {0}")]
    WebhookRequest(#[from] RunnerIngressRequestError),
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

#[derive(Debug, Error)]
pub enum RunnerWebhookRuntimeError {
    #[error("failed to bind webhook listener on `{addr}`")]
    Bind {
        addr: std::net::SocketAddr,
        source: io::Error,
    },
    #[error("failed to accept webhook connection")]
    Accept { source: io::Error },
    #[error("failed to install the rustls ring crypto provider")]
    InstallCryptoProvider,
    #[error("failed to read tls certificate `{path}`")]
    ReadTlsCertificate { path: PathBuf, source: io::Error },
    #[error("failed to read tls private key `{path}`")]
    ReadTlsPrivateKey { path: PathBuf, source: io::Error },
    #[error("tls certificate file `{path}` did not contain any certificates")]
    MissingTlsCertificate { path: PathBuf },
    #[error("tls private key file `{path}` did not contain a private key")]
    MissingTlsPrivateKey { path: PathBuf },
    #[error("failed to build rustls server config")]
    BuildTlsConfig { source: rustls::Error },
    #[error("failed tls handshake for webhook connection")]
    TlsHandshake { source: io::Error },
    #[error("failed to serve webhook connection")]
    ServeConnection {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("webhook connection task failed")]
    ConnectionTask { source: JoinError },
}

#[derive(Debug, Error)]
pub enum RunnerIngressRequestError {
    #[error(transparent)]
    Payload(#[from] RunnerWebhookPayloadError),
    #[error(transparent)]
    Routing(#[from] RunnerWebhookRoutingError),
    #[error("durable webhook persistence is not implemented yet")]
    PersistenceNotImplemented,
}

#[derive(Debug, Error)]
pub enum RunnerWebhookPayloadError {
    #[error("request body is not valid json")]
    InvalidJson { source: serde_json::Error },
    #[error("request body must be a json object")]
    ExpectedObject,
    #[error("request body must match the supported row-batch or resolved shape")]
    UnsupportedShape,
    #[error("row-batch request must include integer `length`")]
    MissingLength,
    #[error("row-batch request `length` must match payload size")]
    LengthMismatch,
    #[error("row-batch request must include array `payload`")]
    MissingPayload,
    #[error("row-batch event must be a json object")]
    InvalidRowEvent,
    #[error("row-batch event `source` must be a json object when present")]
    InvalidSource,
    #[error("row-batch event source is missing `{field}`")]
    MissingSourceField { field: &'static str },
    #[error("resolved request must include non-empty `resolved`")]
    InvalidResolved,
}

#[derive(Debug, Error)]
pub enum RunnerWebhookRoutingError {
    #[error("unknown mapping `{mapping_id}`")]
    UnknownMapping { mapping_id: String },
    #[error("row-batch event is missing source metadata for mapping `{mapping_id}`")]
    MissingSource { mapping_id: String },
    #[error(
        "row-batch source database `{database}` does not match mapping `{mapping_id}` expected `{expected}`"
    )]
    SourceDatabaseMismatch {
        mapping_id: String,
        expected: String,
        database: String,
    },
    #[error("row-batch source table `{table}` is not selected by mapping `{mapping_id}`")]
    SourceTableNotMapped { mapping_id: String, table: String },
    #[error(
        "row-batch spans multiple source tables for mapping `{mapping_id}`: `{first}` and `{second}`"
    )]
    MixedSourceTables {
        mapping_id: String,
        first: String,
        second: String,
    },
}
