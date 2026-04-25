mod parser;

use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use sqlx::{
    ConnectOptions,
    postgres::{PgConnectOptions, PgSslMode},
};

use crate::error::RunnerConfigError;

#[derive(Clone, Debug)]
pub(crate) struct RunnerConfig {
    webhook: WebhookConfig,
    reconcile: ReconcileConfig,
    mappings: Vec<MappingConfig>,
}

pub(crate) struct LoadedRunnerConfig {
    path: PathBuf,
    config: RunnerConfig,
}

impl LoadedRunnerConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, RunnerConfigError> {
        let contents = fs::read_to_string(path).map_err(|source| RunnerConfigError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self {
            path: path.to_path_buf(),
            config: parser::parse_runner_config(path, &contents)?,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn config(&self) -> &RunnerConfig {
        &self.config
    }
}

impl RunnerConfig {
    pub(crate) fn webhook(&self) -> &WebhookConfig {
        &self.webhook
    }

    pub(crate) fn reconcile(&self) -> &ReconcileConfig {
        &self.reconcile
    }

    pub(crate) fn mapping_count(&self) -> usize {
        self.mappings.len()
    }

    pub(crate) fn mappings(&self) -> &[MappingConfig] {
        &self.mappings
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MappingConfig {
    pub(super) id: String,
    pub(super) source: SourceConfig,
    pub(super) destination: PostgresTargetConfig,
}

impl MappingConfig {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn source(&self) -> &SourceConfig {
        &self.source
    }

    pub(crate) fn destination(&self) -> &PostgresTargetConfig {
        &self.destination
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SourceConfig {
    pub(super) database: String,
    pub(super) tables: Vec<String>,
}

impl SourceConfig {
    pub(crate) fn database(&self) -> &str {
        &self.database
    }

    pub(crate) fn tables(&self) -> &[String] {
        &self.tables
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PostgresTargetConfig {
    connect_options: PgConnectOptions,
    host: String,
    port: u16,
    database: String,
    target_contract: String,
}

impl PostgresTargetConfig {
    pub(crate) fn from_url(url: &str) -> Result<Self, RunnerConfigError> {
        let connect_options = url.parse::<PgConnectOptions>().map_err(|source| {
            let message = source.to_string();
            let message = message
                .strip_prefix("error with configuration: ")
                .unwrap_or(&message)
                .to_owned();
            RunnerConfigError::InvalidFieldDetail {
                field: "mappings.destination.url",
                message,
            }
        })?;

        Self::from_connect_options(connect_options, "mappings.destination.url")
    }

    pub(crate) fn from_parts(
        host: String,
        port: u16,
        database: String,
        user: String,
        password: String,
        tls: Option<PostgresTlsConfig>,
    ) -> Result<Self, RunnerConfigError> {
        let connect_options = PgConnectOptions::new()
            .host(&host)
            .port(port)
            .database(&database)
            .username(&user)
            .password(&password);
        let connect_options = if let Some(tls) = &tls {
            tls.apply_to(connect_options)
        } else {
            connect_options
        };

        Self::from_connect_options(connect_options, "mappings.destination")
    }

    fn from_connect_options(
        connect_options: PgConnectOptions,
        field: &'static str,
    ) -> Result<Self, RunnerConfigError> {
        if connect_options.get_socket().is_some() || connect_options.get_host().starts_with('/') {
            return Err(RunnerConfigError::InvalidField {
                field,
                message: "must target a TCP host; unix socket destinations are not supported",
            });
        }

        if connect_options.get_host().is_empty() {
            return Err(RunnerConfigError::InvalidField {
                field,
                message: "must include a host",
            });
        }

        let Some(database) = connect_options.get_database() else {
            return Err(RunnerConfigError::InvalidField {
                field,
                message: "must include a database name",
            });
        };

        Ok(Self {
            host: connect_options.get_host().to_owned(),
            port: connect_options.get_port(),
            database: database.to_owned(),
            target_contract: connect_options.to_url_lossy().to_string(),
            connect_options,
        })
    }

    pub(crate) fn connect_options(&self) -> PgConnectOptions {
        self.connect_options.clone()
    }

    pub(crate) fn endpoint_label(&self) -> String {
        format!("{}:{}/{}", self.host, self.port, self.database)
    }

    pub(crate) fn same_target_contract(&self, other: &Self) -> bool {
        self.target_contract == other.target_contract
    }

    pub(crate) fn database(&self) -> &str {
        &self.database
    }

    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PostgresTlsConfig {
    pub(super) mode: PostgresTlsMode,
    pub(super) ca_cert_path: Option<PathBuf>,
    pub(super) client_cert_path: Option<PathBuf>,
    pub(super) client_key_path: Option<PathBuf>,
}

impl PostgresTlsConfig {
    fn apply_to(&self, mut options: PgConnectOptions) -> PgConnectOptions {
        options = options.ssl_mode(self.mode.into());
        if let Some(path) = &self.ca_cert_path {
            options = options.ssl_root_cert(path);
        }
        if let Some(path) = &self.client_cert_path {
            options = options.ssl_client_cert(path);
        }
        if let Some(path) = &self.client_key_path {
            options = options.ssl_client_key(path);
        }

        options
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PostgresTlsMode {
    Require,
    VerifyCa,
    VerifyFull,
}

impl From<PostgresTlsMode> for PgSslMode {
    fn from(value: PostgresTlsMode) -> Self {
        match value {
            PostgresTlsMode::Require => PgSslMode::Require,
            PostgresTlsMode::VerifyCa => PgSslMode::VerifyCa,
            PostgresTlsMode::VerifyFull => PgSslMode::VerifyFull,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WebhookConfig {
    pub(super) bind_addr: SocketAddr,
    pub(super) transport: WebhookTransport,
}

impl WebhookConfig {
    pub(crate) fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub(crate) fn effective_mode(&self) -> &'static str {
        self.transport.effective_mode()
    }

    pub(crate) fn tls(&self) -> Option<&TlsConfig> {
        self.transport.tls()
    }
}

#[derive(Clone, Debug)]
pub(crate) enum WebhookTransport {
    Http,
    Https(TlsConfig),
}

impl WebhookTransport {
    pub(crate) fn tls(&self) -> Option<&TlsConfig> {
        match self {
            Self::Http => None,
            Self::Https(tls) => Some(tls),
        }
    }

    pub(crate) fn effective_mode(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https(tls) if tls.client_ca_path.is_some() => "https+mtls",
            Self::Https(_) => "https",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TlsConfig {
    pub(super) cert_path: PathBuf,
    pub(super) key_path: PathBuf,
    pub(super) client_ca_path: Option<PathBuf>,
}

impl TlsConfig {
    pub(crate) fn cert_path(&self) -> &Path {
        &self.cert_path
    }

    pub(crate) fn key_path(&self) -> &Path {
        &self.key_path
    }

    pub(crate) fn client_ca_path(&self) -> Option<&Path> {
        self.client_ca_path.as_deref()
    }

    pub(crate) fn material_label(&self) -> String {
        match &self.client_ca_path {
            Some(client_ca_path) => format!(
                "{}+{}+{}",
                self.cert_path.display(),
                self.key_path.display(),
                client_ca_path.display()
            ),
            None => format!("{}+{}", self.cert_path.display(), self.key_path.display()),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ReconcileConfig {
    pub(super) interval_secs: u64,
}

impl ReconcileConfig {
    pub(crate) fn interval_secs(&self) -> u64 {
        self.interval_secs
    }
}
