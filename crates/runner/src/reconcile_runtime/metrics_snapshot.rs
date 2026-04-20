use std::collections::BTreeSet;

use sqlx::{PgConnection, Row};

use crate::{
    error::RunnerReconcileRuntimeError, helper_plan::HelperShadowTablePlan,
    metrics::TableMetricsSnapshot, runtime_plan::MappingRuntimePlan, sql_name::SqlIdentifier,
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(super) async fn load_mapping_table_metrics_snapshot(
    postgres: &mut PgConnection,
    mapping: &MappingRuntimePlan,
) -> Result<Vec<TableMetricsSnapshot>, RunnerReconcileRuntimeError> {
    let mut snapshots = Vec::new();
    let mut seen_tables = BTreeSet::new();

    for table in mapping
        .reconcile_upsert_tables()
        .iter()
        .chain(mapping.reconcile_delete_tables())
    {
        let destination_table = table.source_table().label();
        if !seen_tables.insert(destination_table.clone()) {
            continue;
        }

        let state_row = sqlx::query(
            format!(
                "SELECT last_error IS NOT NULL AS has_reconcile_error,
                        EXTRACT(EPOCH FROM last_successful_sync_time)::double precision
                            AS last_success_unixtime_seconds
                 FROM {HELPER_SCHEMA}.table_sync_state
                 WHERE mapping_id = $1
                   AND source_table_name = $2"
            )
            .as_str(),
        )
        .bind(mapping.mapping_id())
        .bind(&destination_table)
        .fetch_optional(&mut *postgres)
        .await
        .map_err(|source| RunnerReconcileRuntimeError::ReadMetricsSnapshot {
            mapping_id: mapping.mapping_id().to_owned(),
            database: mapping.destination().database().to_owned(),
            table: destination_table.clone(),
            source,
        })?
        .ok_or_else(|| RunnerReconcileRuntimeError::MissingTableTrackingState {
            mapping_id: mapping.mapping_id().to_owned(),
            database: mapping.destination().database().to_owned(),
            table: destination_table.clone(),
        })?;

        snapshots.push(TableMetricsSnapshot {
            destination_table,
            shadow_rows: count_rows(postgres, mapping, table, render_helper_table_name(table))
                .await?,
            real_rows: count_rows(postgres, mapping, table, table.source_table().to_string())
                .await?,
            has_reconcile_error: state_row.get("has_reconcile_error"),
            last_success_unixtime_seconds: state_row.get("last_success_unixtime_seconds"),
        });
    }

    Ok(snapshots)
}

async fn count_rows(
    postgres: &mut PgConnection,
    mapping: &MappingRuntimePlan,
    table: &HelperShadowTablePlan,
    rendered_table_name: String,
) -> Result<u64, RunnerReconcileRuntimeError> {
    let destination_table = table.source_table().label();
    let row_count =
        sqlx::query(format!("SELECT count(*) AS row_count FROM {rendered_table_name}").as_str())
            .fetch_one(&mut *postgres)
            .await
            .map_err(|source| RunnerReconcileRuntimeError::ReadMetricsSnapshot {
                mapping_id: mapping.mapping_id().to_owned(),
                database: mapping.destination().database().to_owned(),
                table: destination_table.clone(),
                source,
            })?
            .get::<i64, _>("row_count");

    Ok(row_count
        .try_into()
        .expect("count(*) should never be negative"))
}

fn render_helper_table_name(table: &HelperShadowTablePlan) -> String {
    format!(
        "{}.{}",
        SqlIdentifier::new(HELPER_SCHEMA),
        SqlIdentifier::new(table.helper_table_name())
    )
}
