mod cockroach_export;
mod postgres_export;
mod report;

use std::{
    collections::BTreeSet,
    fmt::{self, Display, Formatter},
    io,
    path::{Path, PathBuf},
};

use thiserror::Error;

use crate::{
    config::LoadedRunnerConfig,
    sql_name::{QualifiedTableName, SqlIdentifier},
    validated_schema::{
        ColumnSchema, ForeignKeyAction, ForeignKeyShape, IndexColumnShape, IndexShape,
        PrimaryKeyShape, SortDirection, TableSchema, UniqueConstraintShape, ValidatedSchema,
    },
};

pub(crate) use report::SchemaMismatchError;
use report::{SchemaMismatch, SchemaSide};

#[derive(Debug, Error)]
pub enum SchemaCompareError {
    #[error("failed to read {format} schema file `{path}`")]
    ReadFile {
        format: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    #[error("missing mapping `{mapping_id}` in config `{config_path}`")]
    MissingMapping {
        config_path: String,
        mapping_id: String,
    },
    #[error("failed to parse {format} schema file `{path}`: {message}")]
    ParseFile {
        format: &'static str,
        path: PathBuf,
        message: String,
    },
    #[error(transparent)]
    Mismatch(#[from] SchemaMismatchError),
}

pub(crate) fn compare_mapping_exports(
    loaded_config: &LoadedRunnerConfig,
    mapping_id: &str,
    cockroach_schema_path: &Path,
    postgres_schema_path: &Path,
) -> Result<SchemaCompareSummary, SchemaCompareError> {
    let validated = validate_mapping_exports(
        loaded_config,
        mapping_id,
        cockroach_schema_path,
        postgres_schema_path,
    )?;

    Ok(SchemaCompareSummary {
        mapping_id: validated.mapping_id,
        compared_tables: validated.selected_tables.len(),
        ignored_tables: validated.ignored_tables,
    })
}

pub(crate) fn validate_mapping_exports(
    loaded_config: &LoadedRunnerConfig,
    mapping_id: &str,
    cockroach_schema_path: &Path,
    postgres_schema_path: &Path,
) -> Result<ValidatedMappingSchema, SchemaCompareError> {
    let mapping =
        loaded_config
            .config()
            .mapping(mapping_id)
            .ok_or_else(|| SchemaCompareError::MissingMapping {
                config_path: loaded_config.path().display().to_string(),
                mapping_id: mapping_id.to_owned(),
            })?;

    let selected_tables = mapping
        .source()
        .tables()
        .iter()
        .map(|value| QualifiedTableName::from_config(value))
        .collect::<Vec<_>>();
    let selected_table_set = selected_tables.iter().cloned().collect::<BTreeSet<_>>();

    let cockroach_schema = cockroach_export::parse_file(cockroach_schema_path)?;
    let postgres_schema = postgres_export::parse_file(postgres_schema_path)?;
    compare_selected_tables(&selected_table_set, &cockroach_schema, &postgres_schema)?;

    let ignored_tables = cockroach_schema
        .tables()
        .keys()
        .chain(postgres_schema.tables().keys())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|table| !selected_table_set.contains(table))
        .count();

    Ok(ValidatedMappingSchema {
        mapping_id: mapping_id.to_owned(),
        selected_tables,
        postgres_schema: postgres_schema.selected(&selected_table_set),
        ignored_tables,
    })
}

pub struct SchemaCompareSummary {
    mapping_id: String,
    compared_tables: usize,
    ignored_tables: usize,
}

impl Display for SchemaCompareSummary {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "schema compatible: mapping={} tables={} ignored_tables={}",
            self.mapping_id, self.compared_tables, self.ignored_tables
        )
    }
}

pub(crate) struct ValidatedMappingSchema {
    pub(crate) mapping_id: String,
    pub(crate) selected_tables: Vec<QualifiedTableName>,
    pub(crate) postgres_schema: ValidatedSchema,
    pub(crate) ignored_tables: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypeFamily {
    String,
    Integer,
    Boolean,
    TimestampWithTimeZone,
}

pub(super) fn apply_statement(
    schema: &mut ValidatedSchema,
    statement: &str,
    path: &Path,
    format: &'static str,
) -> Result<(), SchemaCompareError> {
    let statement = statement.trim();
    if statement.is_empty()
        || statement == "SET"
        || statement.starts_with("SET ")
        || statement.starts_with("SELECT ")
        || statement.starts_with("ALTER TABLE ")
            && statement.contains(" VALIDATE CONSTRAINT ")
    {
        return Ok(());
    }

    if statement.starts_with("CREATE TABLE ") {
        return apply_create_table(schema, statement, path, format);
    }

    if statement.starts_with("ALTER TABLE ") {
        return apply_alter_table(schema, statement, path, format);
    }

    if statement.starts_with("CREATE INDEX ") || statement.starts_with("CREATE UNIQUE INDEX ") {
        return apply_create_index(schema, statement, path, format);
    }

    Ok(())
}

fn apply_create_table(
    schema: &mut ValidatedSchema,
    statement: &str,
    path: &Path,
    format: &'static str,
) -> Result<(), SchemaCompareError> {
    let rest = statement.trim_start_matches("CREATE TABLE ").trim();
    let open_paren = rest.find('(').ok_or_else(|| SchemaCompareError::ParseFile {
        format,
        path: path.to_path_buf(),
        message: format!("missing column list in statement `{statement}`"),
    })?;
    let close_paren = find_matching_paren(rest, open_paren).ok_or_else(|| {
        SchemaCompareError::ParseFile {
            format,
            path: path.to_path_buf(),
            message: format!("unterminated column list in statement `{statement}`"),
        }
    })?;
    let table_name = QualifiedTableName::from_sql(rest[..open_paren].trim());
    let body = &rest[open_paren + 1..close_paren];
    let mut table_schema = TableSchema::default();

    for item in split_top_level_csv(body) {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }

        if item.starts_with("CONSTRAINT ") {
            let (_, constraint_body) = split_first_token(item.trim_start_matches("CONSTRAINT "));
            apply_constraint_body(&mut table_schema, constraint_body, table_name.clone(), path, format)?;
            continue;
        }

        if item.starts_with("PRIMARY KEY ")
            || item.starts_with("UNIQUE ")
            || item.starts_with("UNIQUE INDEX ")
            || item.starts_with("INDEX ")
            || item.starts_with("FOREIGN KEY ")
        {
            apply_constraint_body(&mut table_schema, item, table_name.clone(), path, format)?;
            continue;
        }

        table_schema.push_column(parse_column(item));
    }

    schema.insert_table(table_name, table_schema);
    Ok(())
}

fn apply_alter_table(
    schema: &mut ValidatedSchema,
    statement: &str,
    path: &Path,
    format: &'static str,
) -> Result<(), SchemaCompareError> {
    if statement.contains(" VALIDATE CONSTRAINT ") {
        return Ok(());
    }

    let rest = statement.trim_start_matches("ALTER TABLE ").trim();
    let add_constraint = " ADD CONSTRAINT ";
    let Some(index) = rest.find(add_constraint) else {
        return Ok(());
    };

    let table_name = QualifiedTableName::from_sql(rest[..index].trim());
    let (_, constraint_body) = split_first_token(rest[index + add_constraint.len()..].trim());
    let table_schema = schema.table_mut(&table_name).ok_or_else(|| SchemaCompareError::ParseFile {
        format,
        path: path.to_path_buf(),
        message: format!("constraint references missing table `{}`", table_name.label()),
    })?;
    apply_constraint_body(table_schema, constraint_body, table_name, path, format)
}

fn apply_create_index(
    schema: &mut ValidatedSchema,
    statement: &str,
    path: &Path,
    format: &'static str,
) -> Result<(), SchemaCompareError> {
    let (rest, is_unique) = if let Some(rest) = statement.strip_prefix("CREATE UNIQUE INDEX ") {
        (rest.trim(), true)
    } else {
        (statement.trim_start_matches("CREATE INDEX ").trim(), false)
    };
    let Some(on_index) = rest.find(" ON ") else {
        return Err(SchemaCompareError::ParseFile {
            format,
            path: path.to_path_buf(),
            message: format!("missing ON clause in statement `{statement}`"),
        });
    };

    let after_on = &rest[on_index + 4..];
    let Some(columns_start) = after_on.find('(') else {
        return Err(SchemaCompareError::ParseFile {
            format,
            path: path.to_path_buf(),
            message: format!("missing index column list in statement `{statement}`"),
        });
    };
    let close = find_matching_paren(after_on, columns_start).ok_or_else(|| SchemaCompareError::ParseFile {
        format,
        path: path.to_path_buf(),
        message: format!("unterminated index column list in statement `{statement}`"),
    })?;
    let table_name = after_on[..columns_start]
        .trim()
        .trim_end_matches("USING btree")
        .trim();
    let table_name = QualifiedTableName::from_sql(table_name);
    let columns = parse_index_columns(&after_on[columns_start + 1..close]);
    let table_schema = schema.table_mut(&table_name).ok_or_else(|| SchemaCompareError::ParseFile {
        format,
        path: path.to_path_buf(),
        message: format!("index references missing table `{}`", table_name.label()),
    })?;
    if is_unique {
        table_schema.push_unique_constraint(UniqueConstraintShape::new(
            columns
                .into_iter()
                .map(|column| column.name().clone())
                .collect(),
        ));
    } else {
        table_schema.push_index(IndexShape::new(columns));
    }
    Ok(())
}

fn apply_constraint_body(
    table_schema: &mut TableSchema,
    constraint_body: &str,
    table_name: QualifiedTableName,
    path: &Path,
    format: &'static str,
) -> Result<(), SchemaCompareError> {
    if let Some(columns) = constraint_body
        .strip_prefix("PRIMARY KEY ")
        .and_then(extract_parenthesized)
    {
        table_schema.set_primary_key(PrimaryKeyShape::new(parse_identifier_list(columns)));
        return Ok(());
    }

    if let Some(rest) = constraint_body.strip_prefix("UNIQUE INDEX ") {
        let Some(open) = rest.find('(') else {
            return Err(SchemaCompareError::ParseFile {
                format,
                path: path.to_path_buf(),
                message: format!("missing unique index columns on `{}`", table_name.label()),
            });
        };
        let columns = extract_parenthesized(&rest[open..]).ok_or_else(|| SchemaCompareError::ParseFile {
            format,
            path: path.to_path_buf(),
            message: format!("unterminated unique index columns on `{}`", table_name.label()),
        })?;
        table_schema.push_unique_constraint(UniqueConstraintShape::new(parse_identifier_list(columns)));
        return Ok(());
    }

    if let Some(columns) = constraint_body.strip_prefix("UNIQUE ").and_then(extract_parenthesized)
    {
        table_schema.push_unique_constraint(UniqueConstraintShape::new(parse_identifier_list(columns)));
        return Ok(());
    }

    if let Some(rest) = constraint_body.strip_prefix("INDEX ") {
        let Some(open) = rest.find('(') else {
            return Err(SchemaCompareError::ParseFile {
                format,
                path: path.to_path_buf(),
                message: format!("missing index columns on `{}`", table_name.label()),
            });
        };
        let columns = extract_parenthesized(&rest[open..]).ok_or_else(|| SchemaCompareError::ParseFile {
            format,
            path: path.to_path_buf(),
            message: format!("unterminated index columns on `{}`", table_name.label()),
        })?;
        table_schema.push_index(IndexShape::new(parse_index_columns(columns)));
        return Ok(());
    }

    if let Some(rest) = constraint_body.strip_prefix("FOREIGN KEY ") {
        let Some(source_columns) = extract_parenthesized(rest) else {
            return Err(SchemaCompareError::ParseFile {
                format,
                path: path.to_path_buf(),
                message: format!("missing foreign key source columns on `{}`", table_name.label()),
            });
        };
        let after_source = rest[source_columns.len() + 2..].trim();
        let after_references = after_source.strip_prefix("REFERENCES ").ok_or_else(|| SchemaCompareError::ParseFile {
            format,
            path: path.to_path_buf(),
            message: format!("missing REFERENCES clause on `{}`", table_name.label()),
        })?;
        let Some(reference_open) = after_references.find('(') else {
            return Err(SchemaCompareError::ParseFile {
                format,
                path: path.to_path_buf(),
                message: format!("missing referenced columns on `{}`", table_name.label()),
            });
        };
        let referenced_table = QualifiedTableName::from_sql(after_references[..reference_open].trim());
        let referenced_columns = extract_parenthesized(&after_references[reference_open..]).ok_or_else(|| {
            SchemaCompareError::ParseFile {
                format,
                path: path.to_path_buf(),
                message: format!("unterminated referenced columns on `{}`", table_name.label()),
            }
        })?;
        let trailing = after_references[reference_open + referenced_columns.len() + 2..].trim();
        let on_delete = if trailing.contains("ON DELETE SET NULL") {
            ForeignKeyAction::SetNull
        } else if trailing.contains("ON DELETE CASCADE") {
            ForeignKeyAction::Cascade
        } else if trailing.contains("ON DELETE RESTRICT") {
            ForeignKeyAction::Restrict
        } else {
            ForeignKeyAction::NoAction
        };
        table_schema.push_foreign_key(ForeignKeyShape::new(
            parse_identifier_list(source_columns),
            referenced_table,
            parse_identifier_list(referenced_columns),
            on_delete,
        ));
        return Ok(());
    }

    Ok(())
}

fn parse_column(value: &str) -> ColumnSchema {
    let (column_name, remainder) = split_first_token(value);
    let remainder = remainder.trim();
    let (raw_type, clauses) = split_column_type_and_clauses(remainder);
    let clauses = clauses.to_ascii_uppercase();
    let nullable = !clauses.contains("NOT NULL");

    ColumnSchema::new(
        SqlIdentifier::new(column_name),
        raw_type.to_owned(),
        nullable,
        false,
    )
}

fn compare_selected_tables(
    selected_tables: &BTreeSet<QualifiedTableName>,
    cockroach_schema: &ValidatedSchema,
    postgres_schema: &ValidatedSchema,
) -> Result<(), SchemaCompareError> {
    let mut mismatches = Vec::new();

    for table in selected_tables {
        let Some(cockroach_table) = cockroach_schema.table(table) else {
            mismatches.push(SchemaMismatch::MissingTable {
                side: SchemaSide::Cockroach,
                table: table.label(),
            });
            continue;
        };
        let Some(postgres_table) = postgres_schema.table(table) else {
            mismatches.push(SchemaMismatch::MissingTable {
                side: SchemaSide::Postgres,
                table: table.label(),
            });
            continue;
        };

        let column_names = cockroach_table
            .columns()
            .iter()
            .map(|column| column.name().clone())
            .chain(postgres_table.columns().iter().map(|column| column.name().clone()))
            .collect::<BTreeSet<_>>();

        for column_name in column_names {
            let Some(cockroach_column) = cockroach_table.column(&column_name) else {
                mismatches.push(SchemaMismatch::MissingColumn {
                    side: SchemaSide::Cockroach,
                    table: table.label(),
                    column: column_name.raw().to_owned(),
                });
                continue;
            };
            let Some(postgres_column) = postgres_table.column(&column_name) else {
                mismatches.push(SchemaMismatch::MissingColumn {
                    side: SchemaSide::Postgres,
                    table: table.label(),
                    column: column_name.raw().to_owned(),
                });
                continue;
            };

            match (
                normalize_cockroach_type(cockroach_column.raw_type()),
                normalize_postgres_type(postgres_column.raw_type()),
            ) {
                (Some(cockroach_type), Some(postgres_type)) if cockroach_type == postgres_type => {}
                (Some(_), Some(_)) => mismatches.push(SchemaMismatch::ColumnTypeMismatch {
                    table: table.label(),
                    column: column_name.raw().to_owned(),
                    cockroach_type: cockroach_column.raw_type().to_owned(),
                    postgres_type: postgres_column.raw_type().to_owned(),
                }),
                _ => mismatches.push(SchemaMismatch::UnsupportedTypePair {
                    table: table.label(),
                    column: column_name.raw().to_owned(),
                    cockroach_type: cockroach_column.raw_type().to_owned(),
                    postgres_type: postgres_column.raw_type().to_owned(),
                }),
            }

            if cockroach_column.nullable() != postgres_column.nullable() {
                mismatches.push(SchemaMismatch::NullabilityMismatch {
                    table: table.label(),
                    column: column_name.raw().to_owned(),
                    cockroach_nullable: cockroach_column.nullable(),
                    postgres_nullable: postgres_column.nullable(),
                });
            }
        }

        let cockroach_primary_key = cockroach_table
            .primary_key()
            .map(|shape| render_identifier_columns(shape.columns()))
            .unwrap_or_default();
        let postgres_primary_key = postgres_table
            .primary_key()
            .map(|shape| render_identifier_columns(shape.columns()))
            .unwrap_or_default();
        if cockroach_primary_key != postgres_primary_key {
            mismatches.push(SchemaMismatch::PrimaryKeyMismatch {
                table: table.label(),
                cockroach_columns: cockroach_primary_key,
                postgres_columns: postgres_primary_key,
            });
        }

        let cockroach_unique = render_unique_constraints(cockroach_table.unique_constraints());
        let postgres_unique = render_unique_constraints(postgres_table.unique_constraints());
        if cockroach_unique != postgres_unique {
            mismatches.push(SchemaMismatch::UniqueConstraintMismatch {
                table: table.label(),
                cockroach_constraints: cockroach_unique,
                postgres_constraints: postgres_unique,
            });
        }

        let cockroach_foreign_keys = render_foreign_keys(cockroach_table.foreign_keys());
        let postgres_foreign_keys = render_foreign_keys(postgres_table.foreign_keys());
        if cockroach_foreign_keys != postgres_foreign_keys {
            mismatches.push(SchemaMismatch::ForeignKeyMismatch {
                table: table.label(),
                cockroach_foreign_keys,
                postgres_foreign_keys,
            });
        }

        let cockroach_indexes = render_indexes(cockroach_table.indexes());
        let postgres_indexes = render_indexes(postgres_table.indexes());
        if cockroach_indexes != postgres_indexes {
            mismatches.push(SchemaMismatch::IndexMismatch {
                table: table.label(),
                cockroach_indexes,
                postgres_indexes,
            });
        }
    }

    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(SchemaMismatchError::new(mismatches).into())
    }
}

fn render_identifier_columns(columns: &[SqlIdentifier]) -> Vec<String> {
    columns.iter().map(|value| value.raw().to_owned()).collect()
}

fn render_unique_constraints(constraints: &[UniqueConstraintShape]) -> Vec<Vec<String>> {
    let mut rendered = constraints
        .iter()
        .map(|constraint| render_identifier_columns(constraint.columns()))
        .collect::<Vec<_>>();
    rendered.sort();
    rendered
}

fn render_foreign_keys(foreign_keys: &[ForeignKeyShape]) -> Vec<String> {
    let mut rendered = foreign_keys
        .iter()
        .map(|foreign_key| {
            format!(
                "({})->{}({}) on_delete={}",
                foreign_key
                    .columns()
                    .iter()
                    .map(|value| value.raw())
                    .collect::<Vec<_>>()
                    .join(", "),
                foreign_key.referenced_table().label(),
                foreign_key
                    .referenced_columns()
                    .iter()
                    .map(|value| value.raw())
                    .collect::<Vec<_>>()
                    .join(", "),
                match foreign_key.on_delete() {
                    ForeignKeyAction::NoAction => "no_action",
                    ForeignKeyAction::Cascade => "cascade",
                    ForeignKeyAction::SetNull => "set_null",
                    ForeignKeyAction::Restrict => "restrict",
                }
            )
        })
        .collect::<Vec<_>>();
    rendered.sort();
    rendered
}

fn render_indexes(indexes: &[IndexShape]) -> Vec<String> {
    let mut rendered = indexes
        .iter()
        .map(|index| {
            index
                .columns()
                .iter()
                .map(|column| match column.direction() {
                    SortDirection::Asc => column.name().raw().to_owned(),
                    SortDirection::Desc => format!("{} DESC", column.name().raw()),
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect::<Vec<_>>();
    rendered.sort();
    rendered
}

fn normalize_cockroach_type(value: &str) -> Option<TypeFamily> {
    match value.trim().to_ascii_uppercase().as_str() {
        "STRING" | "VARCHAR" | "TEXT" => Some(TypeFamily::String),
        "INT" | "INT8" | "BIGINT" => Some(TypeFamily::Integer),
        "BOOL" | "BOOLEAN" => Some(TypeFamily::Boolean),
        "TIMESTAMPTZ" => Some(TypeFamily::TimestampWithTimeZone),
        _ => None,
    }
}

fn normalize_postgres_type(value: &str) -> Option<TypeFamily> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == "text"
        || normalized.starts_with("character varying")
        || normalized.starts_with("varchar")
    {
        return Some(TypeFamily::String);
    }

    match normalized.as_str() {
        "integer" | "bigint" | "int" | "int8" => Some(TypeFamily::Integer),
        "bool" | "boolean" => Some(TypeFamily::Boolean),
        "timestamp with time zone" | "timestamptz" => Some(TypeFamily::TimestampWithTimeZone),
        _ => None,
    }
}

fn split_first_token(value: &str) -> (&str, &str) {
    let trimmed = value.trim();
    match trimmed.find(char::is_whitespace) {
        Some(index) => (&trimmed[..index], trimmed[index..].trim()),
        None => (trimmed, ""),
    }
}

fn split_top_level_csv(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                parts.push(value[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(value[start..].trim());
    parts
}

fn split_column_type_and_clauses(value: &str) -> (&str, &str) {
    let uppercase = value.to_ascii_uppercase();
    let split_at = find_first_top_level_keyword(
        &uppercase,
        &[
            " NOT NULL",
            " NULL",
            " DEFAULT ",
            " GENERATED ALWAYS AS ",
            " PRIMARY KEY",
            " UNIQUE",
            " REFERENCES ",
            " CHECK ",
        ],
    )
    .unwrap_or(value.len());

    (value[..split_at].trim(), value[split_at..].trim())
}

fn find_first_top_level_keyword(value: &str, keywords: &[&str]) -> Option<usize> {
    let mut depth = 0i32;
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 => {
                for keyword in keywords {
                    if value[index..].starts_with(keyword) {
                        return Some(index);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_identifier_list(value: &str) -> Vec<SqlIdentifier> {
    split_top_level_csv(value)
        .into_iter()
        .map(|item| split_first_token(item).0)
        .map(SqlIdentifier::new)
        .collect()
}

fn parse_index_columns(value: &str) -> Vec<IndexColumnShape> {
    split_top_level_csv(value)
        .into_iter()
        .map(|item| {
            let mut parts = item.split_whitespace();
            let name = parts.next().expect("index column should have a name");
            let direction = match parts.next().map(|value| value.to_ascii_uppercase()) {
                Some(value) if value == "DESC" => SortDirection::Desc,
                _ => SortDirection::Asc,
            };
            IndexColumnShape::new(SqlIdentifier::new(name), direction)
        })
        .collect()
}

fn extract_parenthesized(value: &str) -> Option<&str> {
    let start = value.find('(')?;
    let end = find_matching_paren(value, start)?;
    Some(&value[start + 1..end])
}

fn find_matching_paren(value: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (offset, character) in value[open_index..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(open_index + offset);
                }
            }
            _ => {}
        }
    }
    None
}
