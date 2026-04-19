use crate::error::BootstrapConfigError;

#[derive(Clone, Debug)]
pub(crate) struct TableName {
    schema: String,
    name: String,
}

impl TableName {
    pub(crate) fn new(schema: String, name: String) -> Self {
        Self { schema, name }
    }

    pub(crate) fn display_name(&self) -> String {
        format!("{}.{}", self.schema, self.name)
    }

    pub(crate) fn schema(&self) -> &str {
        &self.schema
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn sql_reference_in_database(&self, database: &str) -> String {
        format!("{database}.{}", self.display_name())
    }
}

pub(super) fn parse_schema_qualified_table_name(
    value: String,
    field: &'static str,
) -> Result<TableName, BootstrapConfigError> {
    let value = validate_text(value, field)?;
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
            field,
            message: "must be schema-qualified with simple SQL identifiers",
        });
    }

    Ok(TableName::new(schema.to_owned(), name.to_owned()))
}

pub(super) fn validate_text(
    value: String,
    field: &'static str,
) -> Result<String, BootstrapConfigError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BootstrapConfigError::InvalidField {
            field,
            message: "must not be empty",
        });
    }
    Ok(trimmed.to_owned())
}

fn is_simple_identifier(value: &str) -> bool {
    value
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '_')
}
