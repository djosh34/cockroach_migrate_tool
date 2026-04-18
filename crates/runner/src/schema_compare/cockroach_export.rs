use std::path::{Path, PathBuf};

use super::{SchemaCompareError, ValidatedSchema, apply_statement};

pub(super) fn parse_file(path: &Path) -> Result<ValidatedSchema, SchemaCompareError> {
    let contents = std::fs::read_to_string(path).map_err(|source| SchemaCompareError::ReadFile {
        format: "cockroach",
        path: path.to_path_buf(),
        source,
    })?;
    let statements = extract_statements(&contents, path)?;
    let mut schema = ValidatedSchema::default();
    for statement in statements {
        apply_statement(&mut schema, &statement, path, "cockroach")?;
    }
    Ok(schema)
}

fn extract_statements(contents: &str, path: &Path) -> Result<Vec<String>, SchemaCompareError> {
    let mut statements = Vec::new();
    let mut quoted_statement = None::<String>;

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed == "SET" || trimmed == "create_statement" {
            continue;
        }

        if let Some(statement) = quoted_statement.as_mut() {
            statement.push('\n');
            statement.push_str(trimmed);
            if trimmed.ends_with(";\"") {
                statements.push(unquote_statement(statement));
                quoted_statement = None;
            }
            continue;
        }

        if trimmed.starts_with('"') {
            if trimmed.ends_with(";\"") {
                statements.push(unquote_statement(trimmed));
            } else {
                quoted_statement = Some(trimmed.to_owned());
            }
            continue;
        }

        statements.push(trimmed.to_owned());
    }

    if quoted_statement.is_some() {
        return Err(SchemaCompareError::ParseFile {
            format: "cockroach",
            path: PathBuf::from(path),
            message: "unterminated quoted CREATE TABLE statement".to_owned(),
        });
    }

    Ok(statements)
}

fn unquote_statement(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace("\"\"", "\"")
        .trim()
        .to_owned()
}
