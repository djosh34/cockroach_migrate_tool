use sqlx::{Postgres, Transaction};

use crate::{
    error::RunnerReconcileRuntimeError,
    helper_plan::HelperShadowTablePlan,
    runtime_plan::MappingRuntimePlan,
    sql_name::SqlIdentifier,
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(super) async fn apply(
    transaction: &mut Transaction<'_, Postgres>,
    mapping: &MappingRuntimePlan,
    table: &HelperShadowTablePlan,
) -> Result<(), RunnerReconcileRuntimeError> {
    if table.primary_key_columns().is_empty() {
        return Err(RunnerReconcileRuntimeError::MissingUpsertPrimaryKey {
            mapping_id: mapping.mapping_id().to_owned(),
            table: table.source_table().label(),
        });
    }

    sqlx::query(&render_upsert_sql(table))
        .execute(transaction.as_mut())
        .await
        .map(|_| ())
        .map_err(|source| RunnerReconcileRuntimeError::ApplyUpsert {
            mapping_id: mapping.mapping_id().to_owned(),
            table: table.source_table().label(),
            source,
        })
}

fn render_upsert_sql(table: &HelperShadowTablePlan) -> String {
    let writable_columns = table
        .columns()
        .iter()
        .filter(|column| !column.generated())
        .collect::<Vec<_>>();
    let real_table = table.source_table().to_string();
    let helper_table = format!(
        "{}.{}",
        SqlIdentifier::new(HELPER_SCHEMA),
        SqlIdentifier::new(table.helper_table_name())
    );
    let columns = writable_columns
        .iter()
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let selected_columns = writable_columns
        .iter()
        .map(|column| format!("helper.{}", column.name()))
        .collect::<Vec<_>>()
        .join(", ");
    let conflict_target = table
        .primary_key_columns()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let assignments = table
        .columns()
        .iter()
        .filter(|column| !column.generated())
        .filter(|column| !table.primary_key_columns().contains(column.name()))
        .map(|column| format!("{0} = EXCLUDED.{0}", column.name()))
        .collect::<Vec<_>>();
    let on_conflict = if assignments.is_empty() {
        "DO NOTHING".to_owned()
    } else {
        format!("DO UPDATE SET {}", assignments.join(", "))
    };

    format!(
        "INSERT INTO {real_table} ({columns}) \
         SELECT {selected_columns} \
         FROM {helper_table} AS helper \
         ON CONFLICT ({conflict_target}) {on_conflict}"
    )
}
