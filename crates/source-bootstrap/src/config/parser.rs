use std::{collections::BTreeSet, fs, path::Path};

use serde::Deserialize;

use super::{BootstrapConfig, SourceMapping, SourceSelection, TableName, WebhookConfig};
use crate::error::BootstrapConfigError;

pub(super) fn load(path: &Path) -> Result<BootstrapConfig, BootstrapConfigError> {
    let contents = fs::read_to_string(path).map_err(|source| BootstrapConfigError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let raw = serde_yaml::from_str::<RawBootstrapConfig>(&contents).map_err(|source| {
        BootstrapConfigError::ParseFile {
            path: path.to_path_buf(),
            source,
        }
    })?;
    validate(raw)
}

pub(super) fn validate(raw: RawBootstrapConfig) -> Result<BootstrapConfig, BootstrapConfigError> {
    let mappings = validate_mappings(raw.mappings)?;
    Ok(BootstrapConfig {
        cockroach_url: validate_text(raw.cockroach.url, "cockroach.url")?,
        webhook: validate_webhook(raw.webhook)?,
        mappings,
    })
}

fn validate_webhook(raw: RawWebhookConfig) -> Result<WebhookConfig, BootstrapConfigError> {
    let base_url = validate_text(raw.base_url, "webhook.base_url")?;
    if !base_url.starts_with("https://") {
        return Err(BootstrapConfigError::InvalidField {
            field: "webhook.base_url",
            message: "must start with https://",
        });
    }

    Ok(WebhookConfig {
        base_url,
        resolved: validate_text(raw.resolved, "webhook.resolved")?,
    })
}

fn validate_mappings(
    raw_mappings: Vec<RawSourceMapping>,
) -> Result<Vec<SourceMapping>, BootstrapConfigError> {
    if raw_mappings.is_empty() {
        return Err(BootstrapConfigError::InvalidField {
            field: "mappings",
            message: "must contain at least one mapping",
        });
    }

    let mut ids = BTreeSet::new();
    raw_mappings
        .into_iter()
        .map(|raw_mapping| {
            let id = validate_text(raw_mapping.id, "mappings[].id")?;
            if !ids.insert(id.clone()) {
                return Err(BootstrapConfigError::InvalidField {
                    field: "mappings[].id",
                    message: "must be unique",
                });
            }

            Ok(SourceMapping {
                id,
                source: validate_source(raw_mapping.source)?,
            })
        })
        .collect()
}

fn validate_source(raw: RawSourceSelection) -> Result<SourceSelection, BootstrapConfigError> {
    let tables = validate_tables(raw.tables)?;
    Ok(SourceSelection {
        database: validate_text(raw.database, "mappings[].source.database")?,
        tables,
    })
}

fn validate_tables(raw_tables: Vec<String>) -> Result<Vec<TableName>, BootstrapConfigError> {
    if raw_tables.is_empty() {
        return Err(BootstrapConfigError::InvalidField {
            field: "mappings[].source.tables",
            message: "must contain at least one table",
        });
    }

    let mut tables = Vec::with_capacity(raw_tables.len());
    let mut seen = BTreeSet::new();
    for raw_table in raw_tables {
        let table = validate_table_name(raw_table)?;
        if !seen.insert(table.display_name()) {
            return Err(BootstrapConfigError::InvalidField {
                field: "mappings[].source.tables[]",
                message: "must not contain duplicates",
            });
        }
        tables.push(table);
    }
    Ok(tables)
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

fn validate_table_name(value: String) -> Result<TableName, BootstrapConfigError> {
    let value = validate_text(value, "mappings[].source.tables[]")?;
    let mut parts = value.split('.');
    let schema = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();

    if schema.is_empty()
        || name.is_empty()
        || parts.next().is_some()
        || !is_simple_identifier(schema)
        || !is_simple_identifier(name)
    {
        return Err(BootstrapConfigError::InvalidField {
            field: "mappings[].source.tables[]",
            message: "must be schema-qualified with simple SQL identifiers",
        });
    }

    Ok(TableName {
        schema: schema.to_owned(),
        name: name.to_owned(),
    })
}

fn is_simple_identifier(value: &str) -> bool {
    value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[derive(Debug, Deserialize)]
pub(super) struct RawBootstrapConfig {
    cockroach: RawCockroachConfig,
    webhook: RawWebhookConfig,
    mappings: Vec<RawSourceMapping>,
}

#[derive(Debug, Deserialize)]
struct RawCockroachConfig {
    url: String,
}

#[derive(Debug, Deserialize)]
struct RawWebhookConfig {
    base_url: String,
    resolved: String,
}

#[derive(Debug, Deserialize)]
struct RawSourceMapping {
    id: String,
    source: RawSourceSelection,
}

#[derive(Debug, Deserialize)]
struct RawSourceSelection {
    database: String,
    tables: Vec<String>,
}
