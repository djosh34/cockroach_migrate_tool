use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Display, Formatter},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    config::LoadedRunnerConfig,
    error::{RunnerArtifactError, RunnerHelperPlanError},
    schema_compare::validate_mapping_exports,
    sql_name::{QualifiedTableName, SqlIdentifier},
    validated_schema::{ColumnSchema, ValidatedSchema},
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(crate) fn render_helper_plan(
    loaded_config: &LoadedRunnerConfig,
    mapping_id: &str,
    cockroach_schema_path: &Path,
    postgres_schema_path: &Path,
    output_dir: &Path,
) -> Result<HelperPlanArtifacts, RunnerHelperPlanError> {
    let validated_mapping = validate_mapping_exports(
        loaded_config,
        mapping_id,
        cockroach_schema_path,
        postgres_schema_path,
    )?;
    let plan = MappingHelperPlan::from_validated_schema(
        &validated_mapping.mapping_id,
        &validated_mapping.selected_tables,
        &validated_mapping.postgres_schema,
    )?;
    plan.write_to(output_dir)?;

    Ok(HelperPlanArtifacts {
        mapping_id: validated_mapping.mapping_id,
        output_dir: output_dir.join(plan.mapping_id()),
        helper_tables: plan.helper_tables.len(),
        upsert_order: plan.reconcile_order.upsert_order.len(),
        delete_order: plan.reconcile_order.delete_order.len(),
    })
}

pub struct HelperPlanArtifacts {
    mapping_id: String,
    output_dir: PathBuf,
    helper_tables: usize,
    upsert_order: usize,
    delete_order: usize,
}

impl Display for HelperPlanArtifacts {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "helper plan written: mapping={} output={} helper_tables={} upsert_order={} delete_order={}",
            self.mapping_id,
            self.output_dir.display(),
            self.helper_tables,
            self.upsert_order,
            self.delete_order
        )
    }
}

#[derive(Clone)]
pub(crate) struct MappingHelperPlan {
    mapping_id: String,
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
            mapping_id: mapping_id.to_owned(),
            helper_tables,
            reconcile_order: ReconcileOrder::from_schema(mapping_id, selected_tables, schema)?,
        })
    }

    pub(crate) fn mapping_id(&self) -> &str {
        &self.mapping_id
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

    fn write_to(&self, output_dir: &Path) -> Result<(), RunnerArtifactError> {
        let mapping_dir = output_dir.join(&self.mapping_id);
        fs::create_dir_all(&mapping_dir).map_err(|source| {
            RunnerArtifactError::CreateMappingDirectory {
                path: mapping_dir.clone(),
                source,
            }
        })?;

        write_artifact(mapping_dir.join("README.md"), MappingReadme(self))?;
        write_artifact(mapping_dir.join("helper_tables.sql"), HelperTablesSql(self))?;
        write_artifact(
            mapping_dir.join("reconcile_order.txt"),
            ReconcileOrderText(&self.reconcile_order),
        )?;
        Ok(())
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

struct MappingReadme<'a>(&'a MappingHelperPlan);

impl Display for MappingReadme<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "# Helper Plan For `{}`", self.0.mapping_id)?;
        writeln!(f)?;
        writeln!(
            f,
            "This directory contains the rendered helper shadow-table DDL and reconcile ordering for the selected mapping."
        )?;
        writeln!(f)?;
        writeln!(
            f,
            "- `helper_tables.sql`: helper shadow tables in `{HELPER_SCHEMA}`"
        )?;
        writeln!(
            f,
            "- `reconcile_order.txt`: current upsert and delete order"
        )?;
        Ok(())
    }
}

struct HelperTablesSql<'a>(&'a MappingHelperPlan);

impl Display for HelperTablesSql<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        for helper_table in &self.0.helper_tables {
            write!(f, "{}", helper_table.create_shadow_table_sql())?;
        }
        Ok(())
    }
}

struct ReconcileOrderText<'a>(&'a ReconcileOrder);

impl Display for ReconcileOrderText<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "upsert:")?;
        for table in &self.0.upsert_order {
            writeln!(f, "{}", table.label())?;
        }
        writeln!(f, "delete:")?;
        for table in &self.0.delete_order {
            writeln!(f, "{}", table.label())?;
        }
        Ok(())
    }
}

fn write_artifact(path: PathBuf, content: impl Display) -> Result<(), RunnerArtifactError> {
    fs::write(&path, content.to_string())
        .map_err(|source| RunnerArtifactError::WriteFile { path, source })
}
