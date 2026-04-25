use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Deserializer, de};
use serde_yaml::Value;

use crate::{
    config::{
        MappingConfig, PostgresTargetConfig, PostgresTlsConfig, PostgresTlsMode, ReconcileConfig,
        RunnerConfig, SourceConfig, TlsConfig, WebhookConfig, WebhookMode, WebhookTransport,
    },
    error::RunnerConfigError,
};

pub(super) fn parse_runner_config(
    path: &Path,
    contents: &str,
) -> Result<RunnerConfig, RunnerConfigError> {
    let raw = serde_yaml::from_str::<RawRunnerConfig>(contents).map_err(|source| {
        RunnerConfigError::ParseFile {
            path: path.to_path_buf(),
            source,
        }
    })?;
    raw.validate()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRunnerConfig {
    webhook: RawWebhookConfig,
    reconcile: RawReconcileConfig,
    mappings: Vec<RawMappingConfig>,
}

impl RawRunnerConfig {
    fn validate(self) -> Result<RunnerConfig, RunnerConfigError> {
        let mappings = validate_mappings(self.mappings)?;

        Ok(RunnerConfig {
            webhook: self.webhook.validate()?,
            reconcile: self.reconcile.validate()?,
            mappings,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWebhookConfig {
    bind_addr: String,
    #[serde(default)]
    mode: RawWebhookMode,
    tls: Option<RawTlsConfig>,
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
        let transport = match (self.mode, self.tls) {
            (RawWebhookMode::Http, Some(_)) => {
                return Err(RunnerConfigError::InvalidField {
                    field: "webhook.tls",
                    message: "must not be set when webhook.mode is `http`",
                });
            }
            (RawWebhookMode::Http, None) => WebhookTransport::Http,
            (RawWebhookMode::Https, Some(tls)) => WebhookTransport::Https(tls.validate()?),
            (RawWebhookMode::Https, None) => {
                return Err(RunnerConfigError::InvalidField {
                    field: "webhook.tls",
                    message: "must be set when webhook.mode is `https`",
                });
            }
        };

        Ok(WebhookConfig {
            bind_addr,
            transport,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
enum RawWebhookMode {
    Http,
    #[default]
    Https,
}

impl From<RawWebhookMode> for WebhookMode {
    fn from(value: RawWebhookMode) -> Self {
        match value {
            RawWebhookMode::Http => WebhookMode::Http,
            RawWebhookMode::Https => WebhookMode::Https,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTlsConfig {
    cert_path: PathBuf,
    key_path: PathBuf,
}

impl RawTlsConfig {
    fn validate(self) -> Result<TlsConfig, RunnerConfigError> {
        Ok(TlsConfig {
            cert_path: validate_path(self.cert_path, "webhook.tls.cert_path")?,
            key_path: validate_path(self.key_path, "webhook.tls.key_path")?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMappingConfig {
    id: String,
    source: RawSourceConfig,
    destination: RawPostgresTargetConfig,
}

impl RawMappingConfig {
    fn validate(self) -> Result<MappingConfig, RunnerConfigError> {
        Ok(MappingConfig {
            id: validate_text(self.id, "mappings.id")?,
            source: self.source.validate()?,
            destination: self.destination.validate()?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceConfig {
    database: String,
    tables: Vec<String>,
}

impl RawSourceConfig {
    fn validate(self) -> Result<SourceConfig, RunnerConfigError> {
        Ok(SourceConfig {
            database: validate_text(self.database, "mappings.source.database")?,
            tables: validate_tables(self.tables)?,
        })
    }
}

#[derive(Debug)]
enum RawPostgresTargetConfig {
    Mixed,
    Url(RawPostgresTargetUrlConfig),
    Decomposed(RawDecomposedPostgresTargetConfig),
}

impl<'de> Deserialize<'de> for RawPostgresTargetConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let mapping = value
            .as_mapping()
            .ok_or_else(|| de::Error::custom("destination must be a mapping"))?;

        let mut has_url = false;
        let mut has_decomposed_fields = false;

        for key in mapping.keys() {
            let Some(key) = key.as_str() else {
                continue;
            };

            match key {
                "url" => has_url = true,
                "host" | "port" | "database" | "user" | "password" | "tls" => {
                    has_decomposed_fields = true
                }
                _ => {}
            }
        }

        if has_url && has_decomposed_fields {
            return Ok(Self::Mixed);
        }

        if has_url {
            serde_yaml::from_value::<RawPostgresTargetUrlConfig>(value)
                .map(Self::Url)
                .map_err(de::Error::custom)
        } else {
            serde_yaml::from_value::<RawDecomposedPostgresTargetConfig>(value)
                .map(Self::Decomposed)
                .map_err(de::Error::custom)
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPostgresTargetUrlConfig {
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDecomposedPostgresTargetConfig {
    host: String,
    port: u16,
    database: String,
    user: String,
    password: String,
    tls: Option<RawPostgresTlsConfig>,
}

impl RawPostgresTargetConfig {
    fn validate(self) -> Result<PostgresTargetConfig, RunnerConfigError> {
        match self {
            Self::Mixed => Err(RunnerConfigError::InvalidField {
                field: "mappings.destination",
                message:
                    "`url` cannot be combined with `host`, `port`, `database`, `user`, `password`, or `tls`",
            }),
            Self::Url(raw) => {
                PostgresTargetConfig::from_url(&validate_text(raw.url, "mappings.destination.url")?)
            }
            Self::Decomposed(raw) => raw.validate(),
        }
    }
}

impl RawDecomposedPostgresTargetConfig {
    fn validate(self) -> Result<PostgresTargetConfig, RunnerConfigError> {
        PostgresTargetConfig::from_parts(
            validate_text(self.host, "mappings.destination.host")?,
            self.port,
            validate_text(self.database, "mappings.destination.database")?,
            validate_text(self.user, "mappings.destination.user")?,
            validate_text(self.password, "mappings.destination.password")?,
            self.tls.map(RawPostgresTlsConfig::validate).transpose()?,
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPostgresTlsConfig {
    mode: RawPostgresTlsMode,
    ca_cert_path: Option<PathBuf>,
    client_cert_path: Option<PathBuf>,
    client_key_path: Option<PathBuf>,
}

impl RawPostgresTlsConfig {
    fn validate(self) -> Result<PostgresTlsConfig, RunnerConfigError> {
        let ca_cert_path = self
            .ca_cert_path
            .map(|path| validate_path(path, "mappings.destination.tls.ca_cert_path"))
            .transpose()?;
        let client_cert_path = self
            .client_cert_path
            .map(|path| validate_path(path, "mappings.destination.tls.client_cert_path"))
            .transpose()?;
        let client_key_path = self
            .client_key_path
            .map(|path| validate_path(path, "mappings.destination.tls.client_key_path"))
            .transpose()?;

        if self.mode.requires_ca_cert() && ca_cert_path.is_none() {
            return Err(RunnerConfigError::InvalidField {
                field: "mappings.destination.tls.ca_cert_path",
                message: "must be set when mappings.destination.tls.mode verifies the server certificate",
            });
        }

        if client_cert_path.is_some() != client_key_path.is_some() {
            return Err(RunnerConfigError::InvalidField {
                field: "mappings.destination.tls",
                message: "client_cert_path and client_key_path must be set together",
            });
        }

        Ok(PostgresTlsConfig {
            mode: self.mode.into(),
            ca_cert_path,
            client_cert_path,
            client_key_path,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RawPostgresTlsMode {
    Require,
    VerifyCa,
    VerifyFull,
}

impl RawPostgresTlsMode {
    fn requires_ca_cert(&self) -> bool {
        matches!(self, Self::VerifyCa | Self::VerifyFull)
    }
}

impl From<RawPostgresTlsMode> for PostgresTlsMode {
    fn from(value: RawPostgresTlsMode) -> Self {
        match value {
            RawPostgresTlsMode::Require => PostgresTlsMode::Require,
            RawPostgresTlsMode::VerifyCa => PostgresTlsMode::VerifyCa,
            RawPostgresTlsMode::VerifyFull => PostgresTlsMode::VerifyFull,
        }
    }
}

fn validate_mappings(
    raw_mappings: Vec<RawMappingConfig>,
) -> Result<Vec<MappingConfig>, RunnerConfigError> {
    if raw_mappings.is_empty() {
        return Err(RunnerConfigError::InvalidField {
            field: "mappings",
            message: "must contain at least one mapping",
        });
    }

    let mut seen_ids = BTreeSet::new();
    let mut mappings = Vec::with_capacity(raw_mappings.len());
    for raw_mapping in raw_mappings {
        let mapping = raw_mapping.validate()?;
        if !seen_ids.insert(mapping.id.clone()) {
            return Err(RunnerConfigError::InvalidField {
                field: "mappings.id",
                message: "must be unique",
            });
        }
        mappings.push(mapping);
    }

    Ok(mappings)
}

fn validate_tables(values: Vec<String>) -> Result<Vec<String>, RunnerConfigError> {
    if values.is_empty() {
        return Err(RunnerConfigError::InvalidField {
            field: "mappings.source.tables",
            message: "must contain at least one table",
        });
    }

    let mut tables = Vec::with_capacity(values.len());
    let mut seen = BTreeSet::new();
    for value in values {
        let table = validate_table_name(value)?;
        if !seen.insert(table.clone()) {
            return Err(RunnerConfigError::InvalidField {
                field: "mappings.source.tables",
                message: "must not contain duplicates",
            });
        }
        tables.push(table);
    }

    Ok(tables)
}

fn validate_table_name(value: String) -> Result<String, RunnerConfigError> {
    let table = validate_text(value, "mappings.source.tables")?;
    let mut parts = table.split('.');

    match (parts.next(), parts.next(), parts.next()) {
        (Some(schema), Some(name), None) if !schema.is_empty() && !name.is_empty() => Ok(table),
        _ => Err(RunnerConfigError::InvalidField {
            field: "mappings.source.tables",
            message: "entries must use schema.table",
        }),
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
