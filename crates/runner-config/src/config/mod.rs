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
pub struct RunnerConfig {
    webhook: WebhookConfig,
    reconcile: ReconcileConfig,
    mappings: Vec<MappingConfig>,
}

pub struct LoadedRunnerConfig {
    path: PathBuf,
    config: RunnerConfig,
}

impl LoadedRunnerConfig {
    pub fn load(path: &Path) -> Result<Self, RunnerConfigError> {
        let contents = fs::read_to_string(path).map_err(|source| RunnerConfigError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(Self {
            path: path.to_path_buf(),
            config: parser::parse_runner_config(path, &contents)?,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config(&self) -> &RunnerConfig {
        &self.config
    }
}

impl RunnerConfig {
    pub fn webhook(&self) -> &WebhookConfig {
        &self.webhook
    }

    pub fn reconcile(&self) -> &ReconcileConfig {
        &self.reconcile
    }

    pub fn mapping_count(&self) -> usize {
        self.mappings.len()
    }

    pub fn mappings(&self) -> &[MappingConfig] {
        &self.mappings
    }
}

#[derive(Clone, Debug)]
pub struct MappingConfig {
    pub(crate) id: String,
    pub(crate) source: SourceConfig,
    pub(crate) destination: PostgresTargetConfig,
}

impl MappingConfig {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn source(&self) -> &SourceConfig {
        &self.source
    }

    pub fn destination(&self) -> &PostgresTargetConfig {
        &self.destination
    }
}

#[derive(Clone, Debug)]
pub struct SourceConfig {
    pub(crate) database: String,
    pub(crate) tables: Vec<String>,
}

impl SourceConfig {
    pub fn database(&self) -> &str {
        &self.database
    }

    pub fn tables(&self) -> &[String] {
        &self.tables
    }
}

#[derive(Clone, Debug)]
pub struct PostgresTargetConfig {
    connect_options: PgConnectOptions,
    host: String,
    port: u16,
    database: String,
    target_contract: String,
}

impl PostgresTargetConfig {
    pub fn from_url(url: &str) -> Result<Self, RunnerConfigError> {
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

    pub fn from_parts(
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

    pub fn connect_options(&self) -> PgConnectOptions {
        self.connect_options.clone()
    }

    pub fn endpoint_label(&self) -> String {
        format!("{}:{}/{}", self.host, self.port, self.database)
    }

    pub fn same_target_contract(&self, other: &Self) -> bool {
        self.target_contract == other.target_contract
    }

    pub fn database(&self) -> &str {
        &self.database
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PostgresTlsConfig {
    pub(crate) mode: PostgresTlsMode,
    pub(crate) ca_cert_path: Option<PathBuf>,
    pub(crate) client_cert_path: Option<PathBuf>,
    pub(crate) client_key_path: Option<PathBuf>,
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
pub enum PostgresTlsMode {
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
pub struct WebhookConfig {
    pub(crate) bind_addr: SocketAddr,
    pub(crate) transport: WebhookTransport,
}

impl WebhookConfig {
    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub fn tls(&self) -> Option<&TlsConfig> {
        match &self.transport {
            WebhookTransport::Http => None,
            WebhookTransport::Https(tls) => Some(tls),
        }
    }

    pub fn effective_mode(&self) -> &'static str {
        match &self.transport {
            WebhookTransport::Http => "http",
            WebhookTransport::Https(tls) if tls.client_ca_path().is_some() => "https+mtls",
            WebhookTransport::Https(_) => "https",
        }
    }
}

#[derive(Clone, Debug)]
pub enum WebhookTransport {
    Http,
    Https(TlsConfig),
}

#[derive(Clone, Debug)]
pub struct TlsConfig {
    cert_path: PathBuf,
    key_path: PathBuf,
    client_ca_path: Option<PathBuf>,
}

impl TlsConfig {
    pub fn cert_path(&self) -> &Path {
        &self.cert_path
    }

    pub fn key_path(&self) -> &Path {
        &self.key_path
    }

    pub fn client_ca_path(&self) -> Option<&Path> {
        self.client_ca_path.as_deref()
    }

    pub fn material_label(&self) -> String {
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
pub struct ReconcileConfig {
    interval_secs: u64,
}

impl ReconcileConfig {
    pub fn interval_secs(&self) -> u64 {
        self.interval_secs
    }
}
