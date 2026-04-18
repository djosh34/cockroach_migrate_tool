use std::{fs, path::Path};

use serde::Deserialize;

use crate::error::BootstrapConfigError;

#[derive(Clone, Debug)]
pub(crate) struct BootstrapConfig {
    source_url: String,
    webhook_url: String,
    cursor: String,
    tables: Vec<String>,
}

impl BootstrapConfig {
    pub(crate) fn load(path: &Path) -> Result<Self, BootstrapConfigError> {
        let contents =
            fs::read_to_string(path).map_err(|source| BootstrapConfigError::ReadFile {
                path: path.to_path_buf(),
                source,
            })?;
        let raw = serde_yaml::from_str::<RawBootstrapConfig>(&contents).map_err(|source| {
            BootstrapConfigError::ParseFile {
                path: path.to_path_buf(),
                source,
            }
        })?;
        raw.validate()
    }

    pub(crate) fn cursor(&self) -> &str {
        &self.cursor
    }

    pub(crate) fn source_url(&self) -> &str {
        &self.source_url
    }

    pub(crate) fn tables(&self) -> &[String] {
        &self.tables
    }

    pub(crate) fn webhook_url(&self) -> &str {
        &self.webhook_url
    }
}

#[derive(Debug, Deserialize)]
struct RawBootstrapConfig {
    source: RawSourceConfig,
}

impl RawBootstrapConfig {
    fn validate(self) -> Result<BootstrapConfig, BootstrapConfigError> {
        self.source.validate()
    }
}

#[derive(Debug, Deserialize)]
struct RawSourceConfig {
    url: String,
    webhook_url: String,
    cursor: String,
    tables: Vec<String>,
}

impl RawSourceConfig {
    fn validate(self) -> Result<BootstrapConfig, BootstrapConfigError> {
        let tables = validate_tables(self.tables)?;
        let webhook_url = validate_text(self.webhook_url, "source.webhook_url")?;
        if !webhook_url.starts_with("https://") {
            return Err(BootstrapConfigError::InvalidField {
                field: "source.webhook_url",
                message: "must start with https://",
            });
        }

        Ok(BootstrapConfig {
            source_url: validate_text(self.url, "source.url")?,
            webhook_url,
            cursor: validate_text(self.cursor, "source.cursor")?,
            tables,
        })
    }
}

fn validate_tables(tables: Vec<String>) -> Result<Vec<String>, BootstrapConfigError> {
    if tables.is_empty() {
        return Err(BootstrapConfigError::InvalidField {
            field: "source.tables",
            message: "must contain at least one table",
        });
    }

    tables
        .into_iter()
        .map(|table| validate_text(table, "source.tables[]"))
        .collect()
}

fn validate_text(value: String, field: &'static str) -> Result<String, BootstrapConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BootstrapConfigError::InvalidField {
            field,
            message: "must not be empty",
        });
    }

    Ok(trimmed.to_owned())
}
