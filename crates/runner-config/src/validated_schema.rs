use std::collections::BTreeMap;

use crate::sql_name::{QualifiedTableName, SqlIdentifier};

#[derive(Clone, Debug, Default)]
pub struct ValidatedSchema {
    tables: BTreeMap<QualifiedTableName, TableSchema>,
}

impl ValidatedSchema {
    pub fn table(&self, name: &QualifiedTableName) -> Option<&TableSchema> {
        self.tables.get(name)
    }

    pub fn insert_table(&mut self, name: QualifiedTableName, table: TableSchema) {
        self.tables.insert(name, table);
    }
}

#[derive(Clone, Debug, Default)]
pub struct TableSchema {
    columns: Vec<ColumnSchema>,
    primary_key: Option<PrimaryKeyShape>,
    foreign_keys: Vec<ForeignKeyShape>,
}

impl TableSchema {
    pub fn columns(&self) -> &[ColumnSchema] {
        &self.columns
    }

    pub fn primary_key(&self) -> Option<&PrimaryKeyShape> {
        self.primary_key.as_ref()
    }

    pub fn foreign_keys(&self) -> &[ForeignKeyShape] {
        &self.foreign_keys
    }

    pub fn push_column(&mut self, column: ColumnSchema) {
        self.columns.push(column);
    }

    pub fn set_primary_key(&mut self, primary_key: PrimaryKeyShape) {
        self.primary_key = Some(primary_key);
    }

    pub fn push_foreign_key(&mut self, foreign_key: ForeignKeyShape) {
        self.foreign_keys.push(foreign_key);
    }
}

#[derive(Clone, Debug)]
pub struct ColumnSchema {
    name: SqlIdentifier,
    raw_type: String,
    nullable: bool,
    generated: bool,
}

impl ColumnSchema {
    pub fn new(name: SqlIdentifier, raw_type: String, nullable: bool, generated: bool) -> Self {
        Self {
            name,
            raw_type,
            nullable,
            generated,
        }
    }

    pub fn name(&self) -> &SqlIdentifier {
        &self.name
    }

    pub fn raw_type(&self) -> &str {
        &self.raw_type
    }

    pub fn nullable(&self) -> bool {
        self.nullable
    }

    pub fn generated(&self) -> bool {
        self.generated
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrimaryKeyShape {
    columns: Vec<SqlIdentifier>,
}

impl PrimaryKeyShape {
    pub fn new(columns: Vec<SqlIdentifier>) -> Self {
        Self { columns }
    }

    pub fn columns(&self) -> &[SqlIdentifier] {
        &self.columns
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ForeignKeyShape {
    referenced_table: QualifiedTableName,
    on_delete: ForeignKeyAction,
}

impl ForeignKeyShape {
    pub fn new(
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

    pub fn referenced_table(&self) -> &QualifiedTableName {
        &self.referenced_table
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ForeignKeyAction {
    NoAction,
    Cascade,
    SetNull,
    Restrict,
}
