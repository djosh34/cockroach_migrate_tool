use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use percent_encoding::{AsciiSet, CONTROLS, utf8_percent_encode};
use serde::Deserialize;

use super::{
    BootstrapConfig, SourceMapping, WebhookConfig,
    cockroach::SourceSelection,
    table_name::{TableName, parse_schema_qualified_table_name, validate_text},
};
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
    validate(raw, path.parent().unwrap_or_else(|| Path::new(".")))
}

fn validate(
    raw: RawBootstrapConfig,
    config_dir: &Path,
) -> Result<BootstrapConfig, BootstrapConfigError> {
    let mappings = validate_mappings(raw.mappings)?;
    Ok(BootstrapConfig {
        cockroach_url: validate_text(raw.cockroach.url, "cockroach.url")?,
        webhook: validate_webhook(raw.webhook, config_dir)?,
        mappings,
    })
}

fn validate_webhook(
    raw: RawWebhookConfig,
    config_dir: &Path,
) -> Result<WebhookConfig, BootstrapConfigError> {
    let base_url = validate_text(raw.base_url, "webhook.base_url")?;
    if !base_url.starts_with("https://") {
        return Err(BootstrapConfigError::InvalidField {
            field: "webhook.base_url",
            message: "must start with https://",
        });
    }
    let ca_cert_path = resolve_config_path(
        validate_path(raw.ca_cert_path, "webhook.ca_cert_path")?,
        config_dir,
    );
    let ca_cert_bytes =
        fs::read(&ca_cert_path).map_err(|source| BootstrapConfigError::ReadWebhookCaCert {
            path: ca_cert_path.clone(),
            source,
        })?;

    Ok(WebhookConfig {
        base_url,
        ca_cert_query: encode_ca_cert_query(&ca_cert_bytes),
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

fn validate_path(value: PathBuf, field: &'static str) -> Result<PathBuf, BootstrapConfigError> {
    if value.as_os_str().is_empty() {
        return Err(BootstrapConfigError::InvalidField {
            field,
            message: "must not be empty",
        });
    }

    Ok(value)
}

fn resolve_config_path(path: PathBuf, config_dir: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        config_dir.join(path)
    }
}

const CA_CERT_QUERY_ESCAPE: &AsciiSet = &CONTROLS.add(b'+').add(b'/').add(b'=');

fn encode_ca_cert_query(bytes: &[u8]) -> String {
    let encoded = STANDARD.encode(bytes);
    utf8_percent_encode(&encoded, CA_CERT_QUERY_ESCAPE).to_string()
}

fn validate_table_name(value: String) -> Result<TableName, BootstrapConfigError> {
    parse_schema_qualified_table_name(value, "mappings[].source.tables[]")
}

#[derive(Debug, Deserialize)]
struct RawBootstrapConfig {
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
    ca_cert_path: PathBuf,
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
