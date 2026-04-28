use runner_config::SqlIdentifier;
use sqlx::{Postgres, Transaction};

use crate::{helper_plan::HelperShadowTablePlan, runtime_plan::MappingRuntimePlan};

use super::ReconcileApplyFailure;

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(super) async fn apply(
    transaction: &mut Transaction<'_, Postgres>,
    _mapping: &MappingRuntimePlan,
    table: &HelperShadowTablePlan,
) -> Result<(), ReconcileApplyFailure> {
    if table.primary_key_columns().is_empty() {
        return Err(ReconcileApplyFailure::missing_delete_primary_key(
            table.source_table().label(),
        ));
    }

    sqlx::query(&render_delete_sql(table))
        .execute(transaction.as_mut())
        .await
        .map(|_| ())
        .map_err(|source| ReconcileApplyFailure::apply_delete(table.source_table().label(), source))
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
