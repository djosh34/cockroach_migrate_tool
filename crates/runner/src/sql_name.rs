use std::fmt::{self, Display, Formatter};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct SqlIdentifier {
    raw: String,
}

impl SqlIdentifier {
    pub(crate) fn new(value: &str) -> Self {
        Self {
            raw: unquote_identifier(value.trim()),
        }
    }

    pub(crate) fn raw(&self) -> &str {
        &self.raw
    }
}

impl Display for SqlIdentifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "\"{}\"", self.raw.replace('"', "\"\""))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct QualifiedTableName {
    schema: SqlIdentifier,
    table: SqlIdentifier,
}

impl QualifiedTableName {
    pub(crate) fn new(schema: SqlIdentifier, table: SqlIdentifier) -> Self {
        Self { schema, table }
    }

    pub(crate) fn from_config(value: &str) -> Self {
        let (schema, table) = value
            .split_once('.')
            .expect("validated config should only contain schema-qualified tables");

        Self::new(SqlIdentifier::new(schema), SqlIdentifier::new(table))
    }

    pub(crate) fn schema(&self) -> &SqlIdentifier {
        &self.schema
    }

    pub(crate) fn table(&self) -> &SqlIdentifier {
        &self.table
    }

    pub(crate) fn label(&self) -> String {
        format!("{}.{}", self.schema.raw(), self.table.raw())
    }
}

impl Display for QualifiedTableName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.schema, self.table)
    }
}

fn unquote_identifier(value: &str) -> String {
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        value[1..value.len() - 1].replace("\"\"", "\"")
    } else {
        value.to_owned()
    }
}
