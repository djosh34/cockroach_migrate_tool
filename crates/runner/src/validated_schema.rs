use std::collections::{BTreeMap, BTreeSet};

use crate::sql_name::{QualifiedTableName, SqlIdentifier};

#[derive(Clone, Debug, Default)]
pub(crate) struct ValidatedSchema {
    tables: BTreeMap<QualifiedTableName, TableSchema>,
}

impl ValidatedSchema {
    pub(crate) fn tables(&self) -> &BTreeMap<QualifiedTableName, TableSchema> {
        &self.tables
    }

    pub(crate) fn table(&self, name: &QualifiedTableName) -> Option<&TableSchema> {
        self.tables.get(name)
    }

    pub(crate) fn table_mut(&mut self, name: &QualifiedTableName) -> Option<&mut TableSchema> {
        self.tables.get_mut(name)
    }

    pub(crate) fn insert_table(&mut self, name: QualifiedTableName, table: TableSchema) {
        self.tables.insert(name, table);
    }

    pub(crate) fn selected(&self, selected_tables: &BTreeSet<QualifiedTableName>) -> Self {
        let tables = self
            .tables
            .iter()
            .filter(|(name, _)| selected_tables.contains(*name))
            .map(|(name, table)| (name.clone(), table.clone()))
            .collect();
        Self { tables }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct TableSchema {
    columns: Vec<ColumnSchema>,
    primary_key: Option<PrimaryKeyShape>,
    foreign_keys: Vec<ForeignKeyShape>,
    unique_constraints: Vec<UniqueConstraintShape>,
    indexes: Vec<IndexShape>,
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

    pub(crate) fn unique_constraints(&self) -> &[UniqueConstraintShape] {
        &self.unique_constraints
    }

    pub(crate) fn indexes(&self) -> &[IndexShape] {
        &self.indexes
    }

    pub(crate) fn column(&self, name: &SqlIdentifier) -> Option<&ColumnSchema> {
        self.columns.iter().find(|column| column.name == *name)
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

    pub(crate) fn push_unique_constraint(&mut self, unique_constraint: UniqueConstraintShape) {
        self.unique_constraints.push(unique_constraint);
    }

    pub(crate) fn push_index(&mut self, index: IndexShape) {
        self.indexes.push(index);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ColumnSchema {
    name: SqlIdentifier,
    raw_type: String,
    nullable: bool,
}

impl ColumnSchema {
    pub(crate) fn new(name: SqlIdentifier, raw_type: String, nullable: bool) -> Self {
        Self {
            name,
            raw_type,
            nullable,
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
pub(crate) struct UniqueConstraintShape {
    columns: Vec<SqlIdentifier>,
}

impl UniqueConstraintShape {
    pub(crate) fn new(columns: Vec<SqlIdentifier>) -> Self {
        Self { columns }
    }

    pub(crate) fn columns(&self) -> &[SqlIdentifier] {
        &self.columns
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ForeignKeyShape {
    columns: Vec<SqlIdentifier>,
    referenced_table: QualifiedTableName,
    referenced_columns: Vec<SqlIdentifier>,
    on_delete: ForeignKeyAction,
}

impl ForeignKeyShape {
    pub(crate) fn new(
        columns: Vec<SqlIdentifier>,
        referenced_table: QualifiedTableName,
        referenced_columns: Vec<SqlIdentifier>,
        on_delete: ForeignKeyAction,
    ) -> Self {
        Self {
            columns,
            referenced_table,
            referenced_columns,
            on_delete,
        }
    }

    pub(crate) fn columns(&self) -> &[SqlIdentifier] {
        &self.columns
    }

    pub(crate) fn referenced_table(&self) -> &QualifiedTableName {
        &self.referenced_table
    }

    pub(crate) fn referenced_columns(&self) -> &[SqlIdentifier] {
        &self.referenced_columns
    }

    pub(crate) fn on_delete(&self) -> ForeignKeyAction {
        self.on_delete
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ForeignKeyAction {
    NoAction,
    Cascade,
    SetNull,
    Restrict,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct IndexShape {
    columns: Vec<IndexColumnShape>,
}

impl IndexShape {
    pub(crate) fn new(columns: Vec<IndexColumnShape>) -> Self {
        Self { columns }
    }

    pub(crate) fn columns(&self) -> &[IndexColumnShape] {
        &self.columns
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct IndexColumnShape {
    name: SqlIdentifier,
    direction: SortDirection,
}

impl IndexColumnShape {
    pub(crate) fn new(name: SqlIdentifier, direction: SortDirection) -> Self {
        Self { name, direction }
    }

    pub(crate) fn name(&self) -> &SqlIdentifier {
        &self.name
    }

    pub(crate) fn direction(&self) -> SortDirection {
        self.direction
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum SortDirection {
    Asc,
    Desc,
}
