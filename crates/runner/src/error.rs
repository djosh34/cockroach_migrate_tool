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
    #[error("runtime plan: {0}")]
    RuntimePlan(#[from] RunnerRuntimePlanError),
    #[error("reconcile runtime: {0}")]
    ReconcileRuntime(#[from] RunnerReconcileRuntimeError),
    #[error("webhook runtime: {0}")]
    WebhookRuntime(#[from] RunnerWebhookRuntimeError),
    #[error("webhook request: {0}")]
    WebhookRequest(#[from] RunnerIngressRequestError),
    #[error(transparent)]
    SchemaCompare(#[from] SchemaCompareError),
    #[error("verify: {0}")]
    Verify(#[from] RunnerVerifyError),
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
pub enum RunnerVerifyError {
    #[error(transparent)]
    Artifact(#[from] RunnerArtifactError),
    #[error("mapping `{mapping_id}` is not defined in `{config_path}`")]
    UnknownMapping {
        mapping_id: String,
        config_path: PathBuf,
    },
    #[error("mapping `{mapping_id}` includes invalid table `{table}`; expected schema.table")]
    InvalidMappedTable { mapping_id: String, table: String },
    #[error("failed to start molt verify command `{command}`")]
    SpawnCommand { command: String, source: io::Error },
    #[error("molt verify command `{command}` exited with status `{status}`")]
    CommandFailed { command: String, status: i32 },
    #[error("molt verify for mapping `{mapping_id}` did not emit any summary records")]
    MissingSummary { mapping_id: String },
    #[error("molt verify for mapping `{mapping_id}` did not emit a completion record")]
    MissingCompletion { mapping_id: String },
    #[error("data mismatches detected for mapping `{mapping_id}`: {details}")]
    DataMismatch {
        mapping_id: String,
        details: String,
    },
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
    #[error("failed to seed tracking state for mapping `{mapping_id}` in `{database}`: {source}")]
    SeedTrackingState {
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
pub enum RunnerRuntimePlanError {
    #[error(
        "destination database `{destination}` has conflicting connection contracts for mappings `{first_mapping_id}` and `{second_mapping_id}`"
    )]
    InconsistentDestinationConnection {
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
    #[error("bootstrap output is missing helper metadata for mapping `{mapping_id}`")]
    MissingHelperPlan { mapping_id: String },
    #[error(
        "helper plan for mapping `{mapping_id}` is missing reconcile metadata for selected table `{table}`"
    )]
    MissingReconcileTable { mapping_id: String, table: String },
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
pub enum RunnerReconcileRuntimeError {
    #[error("failed to connect mapping `{mapping_id}` to `{endpoint}` for reconcile: {source}")]
    Connect {
        mapping_id: String,
        endpoint: String,
        source: sqlx::Error,
    },
    #[error(
        "failed to begin reconcile transaction for mapping `{mapping_id}` in `{database}`: {source}"
    )]
    BeginTransaction {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
    #[error(
        "reconcile upsert for mapping `{mapping_id}` real table `{table}` requires primary-key metadata"
    )]
    MissingUpsertPrimaryKey { mapping_id: String, table: String },
    #[error(
        "reconcile delete for mapping `{mapping_id}` real table `{table}` requires primary-key metadata"
    )]
    MissingDeletePrimaryKey { mapping_id: String, table: String },
    #[error(
        "failed to apply reconcile upsert for mapping `{mapping_id}` real table `{table}`: {source}"
    )]
    ApplyUpsert {
        mapping_id: String,
        table: String,
        source: sqlx::Error,
    },
    #[error(
        "failed to apply reconcile delete for mapping `{mapping_id}` real table `{table}`: {source}"
    )]
    ApplyDelete {
        mapping_id: String,
        table: String,
        source: sqlx::Error,
    },
    #[error(
        "failed to roll back reconcile transaction for mapping `{mapping_id}` in `{database}` after a pass error: {source}"
    )]
    Rollback {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
    #[error(
        "failed to update reconcile tracking state for mapping `{mapping_id}` in `{database}`: {source}"
    )]
    UpdateTrackingState {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
    #[error(
        "failed to begin reconcile failure-tracking transaction for mapping `{mapping_id}` in `{database}`: {source}"
    )]
    BeginFailureTrackingTransaction {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
    #[error("stream tracking state row is missing for mapping `{mapping_id}` in `{database}`")]
    MissingTrackingState { mapping_id: String, database: String },
    #[error(
        "table sync tracking state row is missing for mapping `{mapping_id}` in `{database}` table `{table}`"
    )]
    MissingTableTrackingState {
        mapping_id: String,
        database: String,
        table: String,
    },
    #[error(
        "failed to persist reconcile failure-tracking state for mapping `{mapping_id}` in `{database}`: {source}"
    )]
    PersistFailureTrackingState {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
    #[error(
        "failed to commit reconcile failure-tracking transaction for mapping `{mapping_id}` in `{database}`: {source}"
    )]
    CommitFailureTrackingTransaction {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
    #[error(
        "failed to commit reconcile transaction for mapping `{mapping_id}` in `{database}`: {source}"
    )]
    Commit {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
    #[error("reconcile worker task failed")]
    WorkerTask { source: JoinError },
}

#[derive(Debug, Error)]
pub enum RunnerIngressRequestError {
    #[error(transparent)]
    Payload(#[from] RunnerWebhookPayloadError),
    #[error(transparent)]
    Routing(#[from] RunnerWebhookRoutingError),
    #[error(transparent)]
    Persistence(#[from] RunnerWebhookPersistenceError),
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
    #[error("row-batch request payload must include at least one event")]
    EmptyPayload,
    #[error("row-batch event must be a json object")]
    InvalidRowEvent,
    #[error("row-batch event must include `source`")]
    MissingSource,
    #[error("row-batch event `source` must be a json object")]
    InvalidSource,
    #[error("row-batch event source is missing `{field}`")]
    MissingSourceField { field: &'static str },
    #[error("row-batch event must include string `op`")]
    MissingOperation,
    #[error("row-batch event `op` `{op}` is not supported")]
    UnsupportedOperation { op: String },
    #[error("row-batch event must include object `key`")]
    MissingKey,
    #[error("row-batch event `key` must be a json object")]
    InvalidKey,
    #[error("upsert row-batch event must include object `after`")]
    MissingAfter,
    #[error("upsert row-batch event `after` must be a json object")]
    InvalidAfter,
    #[error("resolved request must include non-empty `resolved`")]
    InvalidResolved,
}

#[derive(Debug, Error)]
pub enum RunnerWebhookRoutingError {
    #[error("unknown mapping `{mapping_id}`")]
    UnknownMapping { mapping_id: String },
    #[error("row-batch for mapping `{mapping_id}` must include at least one event")]
    EmptyRowBatch { mapping_id: String },
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

#[derive(Debug, Error)]
pub enum RunnerWebhookPersistenceError {
    #[error("failed to connect mapping `{mapping_id}` to `{endpoint}` for helper persistence: {source}")]
    Connect {
        mapping_id: String,
        endpoint: String,
        source: sqlx::Error,
    },
    #[error("failed to begin helper persistence transaction for mapping `{mapping_id}` in `{database}`: {source}")]
    BeginTransaction {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
    #[error("failed to apply helper persistence for mapping `{mapping_id}` helper table `{helper_table}`: {source}")]
    ApplyMutation {
        mapping_id: String,
        helper_table: String,
        source: sqlx::Error,
    },
    #[error("row-batch helper persistence for mapping `{mapping_id}` helper table `{helper_table}` requires primary-key metadata")]
    MissingPrimaryKey {
        mapping_id: String,
        helper_table: String,
    },
    #[error("row-batch helper persistence for mapping `{mapping_id}` helper table `{helper_table}` is missing required row values")]
    MissingValues {
        mapping_id: String,
        helper_table: String,
    },
    #[error("row-batch helper persistence for mapping `{mapping_id}` helper table `{helper_table}` does not support `{operation}` yet")]
    UnsupportedOperation {
        mapping_id: String,
        helper_table: String,
        operation: &'static str,
    },
    #[error("failed to update stream tracking state for mapping `{mapping_id}` in `{database}`: {source}")]
    UpdateTrackingState {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
    #[error("stream tracking state row is missing for mapping `{mapping_id}` in `{database}`")]
    MissingTrackingState { mapping_id: String, database: String },
    #[error("failed to commit helper persistence transaction for mapping `{mapping_id}` in `{database}`: {source}")]
    Commit {
        mapping_id: String,
        database: String,
        source: sqlx::Error,
    },
}
