mod parser;

use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use sqlx::postgres::{PgConnectOptions, PgSslMode};

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
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) database: String,
    pub(super) user: String,
    pub(super) password: String,
    pub(super) tls: Option<PostgresTlsConfig>,
}

impl PostgresTargetConfig {
    pub(crate) fn connect_options(&self) -> PgConnectOptions {
        let options = PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .database(&self.database)
            .username(&self.user)
            .password(&self.password);

        if let Some(tls) = &self.tls {
            tls.apply_to(options)
        } else {
            options
        }
    }

    pub(crate) fn endpoint_label(&self) -> String {
        format!("{}:{}/{}", self.host, self.port, self.database)
    }

    pub(crate) fn same_target_contract(&self, other: &Self) -> bool {
        self.host == other.host
            && self.port == other.port
            && self.database == other.database
            && self.user == other.user
            && self.password == other.password
            && self.tls == other.tls
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
    pub(super) tls: TlsConfig,
}

impl WebhookConfig {
    pub(crate) fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub(crate) fn tls(&self) -> &TlsConfig {
        &self.tls
    }

    pub(crate) fn tls_material_label(&self) -> String {
        self.tls.material_label()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TlsConfig {
    pub(super) cert_path: PathBuf,
    pub(super) key_path: PathBuf,
}

impl TlsConfig {
    pub(crate) fn cert_path(&self) -> &Path {
        &self.cert_path
    }

    pub(crate) fn key_path(&self) -> &Path {
        &self.key_path
    }

    pub(crate) fn material_label(&self) -> String {
        format!("{}+{}", self.cert_path.display(), self.key_path.display())
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
