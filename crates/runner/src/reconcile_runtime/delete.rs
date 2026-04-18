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
        return Err(RunnerReconcileRuntimeError::MissingDeletePrimaryKey {
            mapping_id: mapping.mapping_id().to_owned(),
            table: table.source_table().label(),
        });
    }

    sqlx::query(&render_delete_sql(table))
        .execute(transaction.as_mut())
        .await
        .map(|_| ())
        .map_err(|source| RunnerReconcileRuntimeError::ApplyDelete {
            mapping_id: mapping.mapping_id().to_owned(),
            table: table.source_table().label(),
            source,
        })
}

fn render_delete_sql(table: &HelperShadowTablePlan) -> String {
    let real_table = table.source_table().to_string();
    let helper_table = format!(
        "{}.{}",
        SqlIdentifier::new(HELPER_SCHEMA),
        SqlIdentifier::new(table.helper_table_name())
    );
    let predicates = table
        .primary_key_columns()
        .iter()
        .map(|column| format!("helper.{0} IS NOT DISTINCT FROM target.{0}", column))
        .collect::<Vec<_>>()
        .join(" AND ");

    format!(
        "DELETE FROM {real_table} AS target \
         WHERE NOT EXISTS (\
             SELECT 1 \
             FROM {helper_table} AS helper \
             WHERE {predicates}\
         )"
    )
}
