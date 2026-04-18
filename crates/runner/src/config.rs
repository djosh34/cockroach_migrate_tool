use std::{
    fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::error::RunnerConfigError;

#[derive(Clone, Debug)]
pub(crate) struct RunnerConfig {
    postgres: PostgresConfig,
    webhook: WebhookConfig,
    reconcile: ReconcileConfig,
}

impl RunnerConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, RunnerConfigError> {
        let contents = fs::read_to_string(path).map_err(|source| RunnerConfigError::ReadFile {
            path: path.to_path_buf(),
            source,
        })?;
        let raw = serde_yaml::from_str::<RawRunnerConfig>(&contents).map_err(|source| {
            RunnerConfigError::ParseFile {
                path: path.to_path_buf(),
                source,
            }
        })?;
        raw.validate()
    }

    pub(crate) fn postgres(&self) -> &PostgresConfig {
        &self.postgres
    }

    pub(crate) fn webhook(&self) -> &WebhookConfig {
        &self.webhook
    }

    pub(crate) fn reconcile(&self) -> &ReconcileConfig {
        &self.reconcile
    }
}

#[derive(Clone, Debug)]
pub(crate) struct PostgresConfig {
    host: String,
    port: u16,
    database: String,
    user: String,
    password: String,
}

impl PostgresConfig {
    pub(crate) fn endpoint_label(&self) -> String {
        format!("{}:{}/{}", self.host, self.port, self.database)
    }

    pub(crate) fn host(&self) -> &str {
        &self.host
    }

    pub(crate) fn port(&self) -> u16 {
        self.port
    }

    pub(crate) fn database(&self) -> &str {
        &self.database
    }

    pub(crate) fn user(&self) -> &str {
        &self.user
    }

    pub(crate) fn password(&self) -> &str {
        &self.password
    }
}

#[derive(Clone, Debug)]
pub(crate) struct WebhookConfig {
    bind_addr: SocketAddr,
    tls_cert_path: PathBuf,
    tls_key_path: PathBuf,
}

impl WebhookConfig {
    pub(crate) fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub(crate) fn tls_cert_path(&self) -> &Path {
        &self.tls_cert_path
    }

    pub(crate) fn tls_key_path(&self) -> &Path {
        &self.tls_key_path
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ReconcileConfig {
    interval_secs: u64,
}

impl ReconcileConfig {
    pub(crate) fn interval_secs(&self) -> u64 {
        self.interval_secs
    }
}

#[derive(Debug, Deserialize)]
struct RawRunnerConfig {
    postgres: RawPostgresConfig,
    webhook: RawWebhookConfig,
    reconcile: RawReconcileConfig,
}

impl RawRunnerConfig {
    fn validate(self) -> Result<RunnerConfig, RunnerConfigError> {
        Ok(RunnerConfig {
            postgres: self.postgres.validate()?,
            webhook: self.webhook.validate()?,
            reconcile: self.reconcile.validate()?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawPostgresConfig {
    host: String,
    port: u16,
    database: String,
    user: String,
    password: String,
}

impl RawPostgresConfig {
    fn validate(self) -> Result<PostgresConfig, RunnerConfigError> {
        Ok(PostgresConfig {
            host: validate_text(self.host, "postgres.host")?,
            port: self.port,
            database: validate_text(self.database, "postgres.database")?,
            user: validate_text(self.user, "postgres.user")?,
            password: validate_text(self.password, "postgres.password")?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawWebhookConfig {
    bind_addr: String,
    tls_cert_path: PathBuf,
    tls_key_path: PathBuf,
}

impl RawWebhookConfig {
    fn validate(self) -> Result<WebhookConfig, RunnerConfigError> {
        let bind_addr =
            self.bind_addr
                .parse()
                .map_err(|source| RunnerConfigError::InvalidSocketAddr {
                    field: "webhook.bind_addr",
                    source,
                })?;
        Ok(WebhookConfig {
            bind_addr,
            tls_cert_path: validate_path(self.tls_cert_path, "webhook.tls_cert_path")?,
            tls_key_path: validate_path(self.tls_key_path, "webhook.tls_key_path")?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawReconcileConfig {
    interval_secs: u64,
}

impl RawReconcileConfig {
    fn validate(self) -> Result<ReconcileConfig, RunnerConfigError> {
        if self.interval_secs == 0 {
            return Err(RunnerConfigError::InvalidField {
                field: "reconcile.interval_secs",
                message: "must be greater than zero",
            });
        }

        Ok(ReconcileConfig {
            interval_secs: self.interval_secs,
        })
    }
}

fn validate_text(value: String, field: &'static str) -> Result<String, RunnerConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RunnerConfigError::InvalidField {
            field,
            message: "must not be empty",
        });
    }

    Ok(trimmed.to_owned())
}

fn validate_path(value: PathBuf, field: &'static str) -> Result<PathBuf, RunnerConfigError> {
    if value.as_os_str().is_empty() {
        return Err(RunnerConfigError::InvalidField {
            field,
            message: "must not be empty",
        });
    }

    Ok(value)
}
