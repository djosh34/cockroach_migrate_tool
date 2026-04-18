use std::path::Path;

use super::{SchemaCompareError, ValidatedSchema, apply_statement};

pub(super) fn parse_file(path: &Path) -> Result<ValidatedSchema, SchemaCompareError> {
    let contents = std::fs::read_to_string(path).map_err(|source| SchemaCompareError::ReadFile {
        format: "postgres",
        path: path.to_path_buf(),
        source,
    })?;
    let statements = extract_statements(&contents);
    let mut schema = ValidatedSchema::default();
    for statement in statements {
        apply_statement(&mut schema, &statement, path, "postgres")?;
    }
    Ok(schema)
}

fn extract_statements(contents: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut buffer = String::new();

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("--") || trimmed.starts_with('\\') {
            continue;
        }

        if !buffer.is_empty() {
            buffer.push(' ');
        }
        buffer.push_str(trimmed);

        if trimmed.ends_with(';') {
            statements.push(buffer.trim().to_owned());
            buffer.clear();
        }
    }

    if !buffer.is_empty() {
        statements.push(buffer.trim().to_owned());
    }

    statements
}
