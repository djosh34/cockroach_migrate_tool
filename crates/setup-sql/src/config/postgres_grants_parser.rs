use std::{
    collections::BTreeSet,
    fs,
    path::Path,
};

use serde::Deserialize;

use super::{PostgresGrantDestination, PostgresGrantMapping, PostgresGrantsConfig, TableName};
use crate::error::BootstrapConfigError;

pub(super) fn load(path: &Path) -> Result<PostgresGrantsConfig, BootstrapConfigError> {
    let contents = fs::read_to_string(path).map_err(|source| BootstrapConfigError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let raw = serde_yaml::from_str::<RawPostgresGrantsConfig>(&contents).map_err(|source| {
        BootstrapConfigError::ParseFile {
            path: path.to_path_buf(),
            source,
        }
    })?;
    validate(raw)
}

fn validate(raw: RawPostgresGrantsConfig) -> Result<PostgresGrantsConfig, BootstrapConfigError> {
    if raw.mappings.is_empty() {
        return Err(BootstrapConfigError::InvalidField {
            field: "mappings",
            message: "must contain at least one mapping",
        });
    }

    let mut ids = BTreeSet::new();
    let mappings = raw
        .mappings
        .into_iter()
        .map(|raw_mapping| {
            let id = validate_text(raw_mapping.id, "mappings[].id")?;
            if !ids.insert(id.clone()) {
                return Err(BootstrapConfigError::InvalidField {
                    field: "mappings[].id",
                    message: "must be unique",
                });
            }

            Ok(PostgresGrantMapping {
                id,
                destination: validate_destination(raw_mapping.destination)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(PostgresGrantsConfig { mappings })
}

fn validate_destination(
    raw: RawPostgresGrantDestination,
) -> Result<PostgresGrantDestination, BootstrapConfigError> {
    Ok(PostgresGrantDestination {
        database: validate_text(raw.database, "mappings[].destination.database")?,
        runtime_role: validate_text(
            raw.runtime_role,
            "mappings[].destination.runtime_role",
        )?,
        tables: validate_tables(raw.tables)?,
    })
}

fn validate_tables(raw_tables: Vec<String>) -> Result<Vec<TableName>, BootstrapConfigError> {
    if raw_tables.is_empty() {
        return Err(BootstrapConfigError::InvalidField {
            field: "mappings[].destination.tables",
            message: "must contain at least one table",
        });
    }

    let mut tables = Vec::with_capacity(raw_tables.len());
    let mut seen = BTreeSet::new();
    for raw_table in raw_tables {
        let table = validate_table_name(raw_table)?;
        if !seen.insert(table.display_name()) {
            return Err(BootstrapConfigError::InvalidField {
                field: "mappings[].destination.tables[]",
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
    let value = validate_text(value, "mappings[].destination.tables[]")?;
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
            field: "mappings[].destination.tables[]",
            message: "must be schema-qualified with simple SQL identifiers",
        });
    }

    Ok(TableName::new(schema.to_owned(), name.to_owned()))
}

fn is_simple_identifier(value: &str) -> bool {
    value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
}

#[derive(Debug, Deserialize)]
struct RawPostgresGrantsConfig {
    mappings: Vec<RawPostgresGrantMapping>,
}

#[derive(Debug, Deserialize)]
struct RawPostgresGrantMapping {
    id: String,
    destination: RawPostgresGrantDestination,
}

#[derive(Debug, Deserialize)]
struct RawPostgresGrantDestination {
    database: String,
    runtime_role: String,
    tables: Vec<String>,
}
