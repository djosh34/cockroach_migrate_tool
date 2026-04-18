mod parser;

use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use sqlx::postgres::PgConnectOptions;

use crate::error::RunnerConfigError;

#[derive(Clone, Debug)]
pub(crate) struct RunnerConfig {
    webhook: WebhookConfig,
    reconcile: ReconcileConfig,
    verify: VerifyConfig,
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

    pub(crate) fn mapping(&self, mapping_id: &str) -> Option<&MappingConfig> {
        self.mappings.iter().find(|mapping| mapping.id() == mapping_id)
    }

    pub(crate) fn verify_label(&self) -> String {
        self.verify.molt.label()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MappingConfig {
    pub(super) id: String,
    pub(super) source: SourceConfig,
    pub(super) destination: DestinationConfig,
}

impl MappingConfig {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn source(&self) -> &SourceConfig {
        &self.source
    }

    pub(crate) fn destination(&self) -> &DestinationConfig {
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
pub(crate) struct DestinationConfig {
    pub(super) connection: PostgresConnectionConfig,
}

impl DestinationConfig {
    pub(crate) fn connection(&self) -> &PostgresConnectionConfig {
        &self.connection
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PostgresConnectionConfig {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) database: String,
    pub(super) user: String,
    pub(super) password: String,
}

impl PostgresConnectionConfig {
    pub(crate) fn connect_options(&self) -> PgConnectOptions {
        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .database(&self.database)
            .username(&self.user)
            .password(&self.password)
    }

    pub(crate) fn endpoint_label(&self) -> String {
        format!("{}:{}/{}", self.host, self.port, self.database)
    }

    pub(crate) fn database(&self) -> &str {
        &self.database
    }

    pub(crate) fn user(&self) -> &str {
        &self.user
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

#[derive(Clone, Debug)]
pub(crate) struct VerifyConfig {
    pub(super) molt: MoltVerifyConfig,
}

#[derive(Clone, Debug)]
pub(crate) struct MoltVerifyConfig {
    pub(super) command: String,
    pub(super) report_dir: PathBuf,
}

impl MoltVerifyConfig {
    pub(crate) fn label(&self) -> String {
        format!("{}@{}", self.command, self.report_dir.display())
    }
}
