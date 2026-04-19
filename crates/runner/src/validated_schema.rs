use std::collections::BTreeMap;

use crate::sql_name::{QualifiedTableName, SqlIdentifier};

#[derive(Clone, Debug, Default)]
pub(crate) struct ValidatedSchema {
    tables: BTreeMap<QualifiedTableName, TableSchema>,
}

impl ValidatedSchema {
    pub(crate) fn table(&self, name: &QualifiedTableName) -> Option<&TableSchema> {
        self.tables.get(name)
    }

    pub(crate) fn insert_table(&mut self, name: QualifiedTableName, table: TableSchema) {
        self.tables.insert(name, table);
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TableSchema {
    columns: Vec<ColumnSchema>,
    primary_key: Option<PrimaryKeyShape>,
    foreign_keys: Vec<ForeignKeyShape>,
}

impl TableSchema {
    pub(crate) fn columns(&self) -> &[ColumnSchema] {
        &self.columns
    }

    pub(crate) fn primary_key(&self) -> Option<&PrimaryKeyShape> {
        self.primary_key.as_ref()
    }

    pub(crate) fn foreign_keys(&self) -> &[ForeignKeyShape] {
        &self.foreign_keys
    }

    pub(crate) fn push_column(&mut self, column: ColumnSchema) {
        self.columns.push(column);
    }

    pub(crate) fn set_primary_key(&mut self, primary_key: PrimaryKeyShape) {
        self.primary_key = Some(primary_key);
    }

    pub(crate) fn push_foreign_key(&mut self, foreign_key: ForeignKeyShape) {
        self.foreign_keys.push(foreign_key);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ColumnSchema {
    name: SqlIdentifier,
    raw_type: String,
    nullable: bool,
    generated: bool,
}

impl ColumnSchema {
    pub(crate) fn new(
        name: SqlIdentifier,
        raw_type: String,
        nullable: bool,
        generated: bool,
    ) -> Self {
        Self {
            name,
            raw_type,
            nullable,
            generated,
        }
    }

    pub(crate) fn name(&self) -> &SqlIdentifier {
        &self.name
    }

    pub(crate) fn raw_type(&self) -> &str {
        &self.raw_type
    }

    pub(crate) fn nullable(&self) -> bool {
        self.nullable
    }

    pub(crate) fn generated(&self) -> bool {
        self.generated
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrimaryKeyShape {
    columns: Vec<SqlIdentifier>,
}

impl PrimaryKeyShape {
    pub(crate) fn new(columns: Vec<SqlIdentifier>) -> Self {
        Self { columns }
    }

    pub(crate) fn columns(&self) -> &[SqlIdentifier] {
        &self.columns
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ForeignKeyShape {
    referenced_table: QualifiedTableName,
    on_delete: ForeignKeyAction,
}

impl ForeignKeyShape {
    pub(crate) fn new(
        _columns: Vec<SqlIdentifier>,
        referenced_table: QualifiedTableName,
        _referenced_columns: Vec<SqlIdentifier>,
        on_delete: ForeignKeyAction,
    ) -> Self {
        Self {
            referenced_table,
            on_delete,
        }
    }

    pub(crate) fn referenced_table(&self) -> &QualifiedTableName {
        &self.referenced_table
    }

}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ForeignKeyAction {
    NoAction,
    Cascade,
    SetNull,
    Restrict,
}
