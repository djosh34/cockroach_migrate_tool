use std::collections::{BTreeMap, BTreeSet};

use runner_config::{ColumnSchema, QualifiedTableName, SqlIdentifier, ValidatedSchema};

use crate::error::RunnerHelperPlanError;

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

#[derive(Clone)]
pub(crate) struct MappingHelperPlan {
    helper_tables: Vec<HelperShadowTablePlan>,
    reconcile_order: ReconcileOrder,
}

impl MappingHelperPlan {
    pub(crate) fn from_validated_schema(
        mapping_id: &str,
        selected_tables: &[QualifiedTableName],
        schema: &ValidatedSchema,
    ) -> Result<Self, RunnerHelperPlanError> {
        let helper_tables = selected_tables
            .iter()
            .map(|table_name| {
                let table = schema.table(table_name).ok_or_else(|| {
                    RunnerHelperPlanError::MissingValidatedTable {
                        mapping_id: mapping_id.to_owned(),
                        table: table_name.label(),
                    }
                })?;
                Ok(HelperShadowTablePlan::from_table(
                    mapping_id,
                    table_name,
                    table.columns(),
                    table
                        .primary_key()
                        .map(|primary_key| primary_key.columns().to_vec())
                        .unwrap_or_default(),
                ))
            })
            .collect::<Result<Vec<_>, RunnerHelperPlanError>>()?;

        Ok(Self {
            helper_tables,
            reconcile_order: ReconcileOrder::from_schema(mapping_id, selected_tables, schema)?,
        })
    }

    pub(crate) fn helper_tables(&self) -> &[HelperShadowTablePlan] {
        &self.helper_tables
    }

    pub(crate) fn reconcile_upsert_order(&self) -> &[QualifiedTableName] {
        &self.reconcile_order.upsert_order
    }

    pub(crate) fn reconcile_delete_order(&self) -> &[QualifiedTableName] {
        &self.reconcile_order.delete_order
    }
}

#[derive(Clone)]
pub(crate) struct HelperShadowTablePlan {
    source_table: QualifiedTableName,
    helper_table_name: String,
    columns: Vec<HelperColumnPlan>,
    primary_key_columns: Vec<SqlIdentifier>,
}

impl HelperShadowTablePlan {
    pub(crate) fn from_table(
        mapping_id: &str,
        source_table: &QualifiedTableName,
        columns: &[ColumnSchema],
        primary_key_columns: Vec<SqlIdentifier>,
    ) -> Self {
        Self {
            source_table: source_table.clone(),
            helper_table_name: format!(
                "{mapping_id}__{}__{}",
                source_table.schema().raw(),
                source_table.table().raw()
            ),
            columns: columns
                .iter()
                .map(HelperColumnPlan::from_schema_column)
                .collect(),
            primary_key_columns,
        }
    }

    pub(crate) fn source_table(&self) -> &QualifiedTableName {
        &self.source_table
    }

    pub(crate) fn helper_table_name(&self) -> &str {
        &self.helper_table_name
    }

    pub(crate) fn columns(&self) -> &[HelperColumnPlan] {
        &self.columns
    }

    pub(crate) fn primary_key_columns(&self) -> &[SqlIdentifier] {
        &self.primary_key_columns
    }

    pub(crate) fn create_shadow_table_sql(&self) -> String {
        let columns = self
            .columns
            .iter()
            .map(|column| format!("    {}", column.render_sql()))
            .collect::<Vec<_>>()
            .join(",\n");

        format!(
            "CREATE TABLE IF NOT EXISTS {}.{} (\n{columns}\n);\n",
            SqlIdentifier::new(HELPER_SCHEMA),
            SqlIdentifier::new(&self.helper_table_name),
        )
    }

    pub(crate) fn create_primary_key_index_sql(&self) -> Option<String> {
        if self.primary_key_columns.is_empty() {
            return None;
        }

        let columns = self
            .primary_key_columns
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        Some(format!(
            "CREATE UNIQUE INDEX IF NOT EXISTS {} ON {}.{} ({columns})",
            SqlIdentifier::new(&format!("{}__pk", self.helper_table_name)),
            SqlIdentifier::new(HELPER_SCHEMA),
            SqlIdentifier::new(&self.helper_table_name),
        ))
    }
}

#[derive(Clone)]
pub(crate) struct HelperColumnPlan {
    name: SqlIdentifier,
    raw_type: String,
    nullable: bool,
    generated: bool,
}

impl HelperColumnPlan {
    fn from_schema_column(column: &ColumnSchema) -> Self {
        Self {
            name: column.name().clone(),
            raw_type: column.raw_type().to_owned(),
            nullable: column.nullable(),
            generated: column.generated(),
        }
    }

    pub(crate) fn name(&self) -> &SqlIdentifier {
        &self.name
    }

    pub(crate) fn generated(&self) -> bool {
        self.generated
    }

    fn render_sql(&self) -> String {
        let nullability = if self.nullable { "" } else { " NOT NULL" };
        format!("{} {}{}", self.name, self.raw_type, nullability)
    }
}

#[derive(Clone)]
pub(crate) struct ReconcileOrder {
    upsert_order: Vec<QualifiedTableName>,
    delete_order: Vec<QualifiedTableName>,
}

impl ReconcileOrder {
    fn from_schema(
        mapping_id: &str,
        selected_tables: &[QualifiedTableName],
        schema: &ValidatedSchema,
    ) -> Result<Self, RunnerHelperPlanError> {
        let table_positions = selected_tables
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, table)| (table, index))
            .collect::<BTreeMap<_, _>>();
        let mut indegree = vec![0usize; selected_tables.len()];
        let mut children = vec![Vec::<usize>::new(); selected_tables.len()];
        let mut edges = BTreeSet::<(usize, usize)>::new();

        for (child_index, table_name) in selected_tables.iter().enumerate() {
            let table = schema.table(table_name).ok_or_else(|| {
                RunnerHelperPlanError::MissingValidatedTable {
                    mapping_id: mapping_id.to_owned(),
                    table: table_name.label(),
                }
            })?;

            for foreign_key in table.foreign_keys() {
                let Some(parent_index) =
                    table_positions.get(foreign_key.referenced_table()).copied()
                else {
                    continue;
                };
                if edges.insert((parent_index, child_index)) {
                    indegree[child_index] += 1;
                    children[parent_index].push(child_index);
                }
            }
        }

        for dependent_tables in &mut children {
            dependent_tables.sort_unstable();
        }

        let mut ready = indegree
            .iter()
            .enumerate()
            .filter_map(|(index, degree)| (*degree == 0).then_some(index))
            .collect::<Vec<_>>();
        let mut upsert_order = Vec::with_capacity(selected_tables.len());

        while let Some(next_index) = ready.first().copied() {
            ready.remove(0);
            upsert_order.push(selected_tables[next_index].clone());

            for child_index in &children[next_index] {
                indegree[*child_index] -= 1;
                if indegree[*child_index] == 0 {
                    ready.push(*child_index);
                    ready.sort_unstable();
                }
            }
        }

        if upsert_order.len() != selected_tables.len() {
            let remaining_tables = indegree
                .iter()
                .enumerate()
                .filter_map(|(index, degree)| {
                    (*degree > 0).then_some(selected_tables[index].label())
                })
                .collect::<Vec<_>>()
                .join(", ");
            return Err(RunnerHelperPlanError::DependencyCycle {
                mapping_id: mapping_id.to_owned(),
                tables: remaining_tables,
            });
        }

        Ok(Self {
            delete_order: upsert_order.iter().rev().cloned().collect(),
            upsert_order,
        })
    }
}
