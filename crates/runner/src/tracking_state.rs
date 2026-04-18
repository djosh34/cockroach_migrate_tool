use sqlx::{Connection, PgConnection, Postgres, Row, Transaction};

use crate::{
    config::PostgresConnectionConfig,
    error::{RunnerReconcileRuntimeError, RunnerWebhookPersistenceError},
    helper_plan::HelperShadowTablePlan,
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(crate) struct ResolvedTrackingTarget {
    pub(crate) mapping_id: String,
    pub(crate) connection: PostgresConnectionConfig,
    pub(crate) resolved_watermark: String,
}

#[derive(Clone, Copy)]
pub(crate) enum ReconcilePhase {
    Upsert,
    Delete,
}

impl std::fmt::Display for ReconcilePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Upsert => write!(f, "upsert"),
            Self::Delete => write!(f, "delete"),
        }
    }
}

pub(crate) struct ReconcileFailure {
    mapping_id: String,
    database: String,
    table: String,
    phase: ReconcilePhase,
    error_detail: String,
}

impl ReconcileFailure {
    pub(crate) fn new(
        mapping_id: String,
        database: String,
        table: String,
        phase: ReconcilePhase,
        error_message: String,
    ) -> Self {
        Self {
            mapping_id,
            database,
            table,
            phase,
            error_detail: error_message,
        }
    }

    fn rendered_error(&self) -> String {
        format!(
            "reconcile {} failed for {}: {}",
            self.phase, self.table, self.error_detail
        )
    }
}

pub(crate) async fn seed_tracking_state(
    postgres: &mut PgConnection,
    mapping_id: &str,
    source_database: &str,
    helper_tables: &[HelperShadowTablePlan],
) -> Result<(), sqlx::Error> {
    sqlx::query(
        format!(
            "INSERT INTO {HELPER_SCHEMA}.stream_state (
                mapping_id,
                source_database,
                stream_status
             )
             VALUES ($1, $2, 'bootstrap_pending')
             ON CONFLICT (mapping_id) DO UPDATE
             SET source_database = EXCLUDED.source_database"
        )
        .as_str(),
    )
    .bind(mapping_id)
    .bind(source_database)
    .execute(&mut *postgres)
    .await?;

    for helper_table in helper_tables {
        sqlx::query(
            format!(
                "INSERT INTO {HELPER_SCHEMA}.table_sync_state (
                    mapping_id,
                    source_table_name,
                    helper_table_name
                 )
                 VALUES ($1, $2, $3)
                 ON CONFLICT (mapping_id, source_table_name) DO UPDATE
                 SET helper_table_name = EXCLUDED.helper_table_name"
            )
            .as_str(),
        )
        .bind(mapping_id)
        .bind(helper_table.source_table().label())
        .bind(helper_table.helper_table_name())
        .execute(&mut *postgres)
        .await?;
    }

    Ok(())
}

pub(crate) async fn persist_resolved_watermark(
    target: ResolvedTrackingTarget,
) -> Result<(), RunnerWebhookPersistenceError> {
    let endpoint = target.connection.endpoint_label();
    let database = target.connection.database().to_owned();
    let mut postgres = PgConnection::connect_with(&target.connection.connect_options())
        .await
        .map_err(|source| RunnerWebhookPersistenceError::Connect {
            mapping_id: target.mapping_id.clone(),
            endpoint,
            source,
        })?;
    let mut transaction = postgres
        .begin()
        .await
        .map_err(|source| RunnerWebhookPersistenceError::BeginTransaction {
            mapping_id: target.mapping_id.clone(),
            database: database.clone(),
            source,
        })?;

    let result = sqlx::query(
        format!(
            "UPDATE {HELPER_SCHEMA}.stream_state
             SET latest_received_resolved_watermark = CASE
                 WHEN latest_received_resolved_watermark IS NULL
                   OR latest_received_resolved_watermark < $2
                 THEN $2
                 ELSE latest_received_resolved_watermark
             END
             WHERE mapping_id = $1"
        )
        .as_str(),
    )
    .bind(&target.mapping_id)
    .bind(&target.resolved_watermark)
    .execute(transaction.as_mut())
    .await
    .map_err(|source| RunnerWebhookPersistenceError::UpdateTrackingState {
        mapping_id: target.mapping_id.clone(),
        database: database.clone(),
        source,
    })?;

    if result.rows_affected() != 1 {
        return Err(RunnerWebhookPersistenceError::MissingTrackingState {
            mapping_id: target.mapping_id,
            database,
        });
    }

    transaction
        .commit()
        .await
        .map_err(|source| RunnerWebhookPersistenceError::Commit {
            mapping_id: target.mapping_id,
            database,
            source,
        })?;
    Ok(())
}

pub(crate) async fn persist_reconcile_success(
    transaction: &mut Transaction<'_, Postgres>,
    mapping_id: &str,
    database: &str,
    helper_tables: &[HelperShadowTablePlan],
) -> Result<(), RunnerReconcileRuntimeError> {
    let stream_row = sqlx::query(
        format!(
            "UPDATE {HELPER_SCHEMA}.stream_state
             SET latest_reconciled_resolved_watermark = CASE
                 WHEN latest_received_resolved_watermark IS NULL
                 THEN latest_reconciled_resolved_watermark
                 WHEN latest_reconciled_resolved_watermark IS NULL
                   OR latest_reconciled_resolved_watermark < latest_received_resolved_watermark
                 THEN latest_received_resolved_watermark
                 ELSE latest_reconciled_resolved_watermark
             END
             WHERE mapping_id = $1
             RETURNING latest_received_resolved_watermark"
        )
        .as_str(),
    )
    .bind(mapping_id)
    .fetch_optional(transaction.as_mut())
    .await
    .map_err(|source| RunnerReconcileRuntimeError::UpdateTrackingState {
        mapping_id: mapping_id.to_owned(),
        database: database.to_owned(),
        source,
    })?
    .ok_or_else(|| RunnerReconcileRuntimeError::MissingTrackingState {
        mapping_id: mapping_id.to_owned(),
        database: database.to_owned(),
    })?;
    let latest_received = stream_row.get::<Option<String>, _>("latest_received_resolved_watermark");

    let Some(latest_received) = latest_received else {
        return Ok(());
    };

    for helper_table in helper_tables {
        let result = sqlx::query(
            format!(
                "UPDATE {HELPER_SCHEMA}.table_sync_state
                 SET last_successful_sync_time = NOW(),
                     last_successful_sync_watermark = $3,
                     last_error = NULL
                 WHERE mapping_id = $1
                   AND source_table_name = $2"
            )
            .as_str(),
        )
        .bind(mapping_id)
        .bind(helper_table.source_table().label())
        .bind(&latest_received)
        .execute(transaction.as_mut())
        .await
        .map_err(|source| RunnerReconcileRuntimeError::UpdateTrackingState {
            mapping_id: mapping_id.to_owned(),
            database: database.to_owned(),
            source,
        })?;

        if result.rows_affected() != 1 {
            return Err(RunnerReconcileRuntimeError::MissingTableTrackingState {
                mapping_id: mapping_id.to_owned(),
                database: database.to_owned(),
                table: helper_table.source_table().label(),
            });
        }
    }

    Ok(())
}

pub(crate) async fn persist_reconcile_failure(
    postgres: &mut PgConnection,
    failure: ReconcileFailure,
) -> Result<(), RunnerReconcileRuntimeError> {
    let mut transaction = postgres
        .begin()
        .await
        .map_err(|source| RunnerReconcileRuntimeError::BeginFailureTrackingTransaction {
            mapping_id: failure.mapping_id.clone(),
            database: failure.database.clone(),
            source,
        })?;

    let result = sqlx::query(
        format!(
            "UPDATE {HELPER_SCHEMA}.table_sync_state
             SET last_error = $3
             WHERE mapping_id = $1
               AND source_table_name = $2"
        )
        .as_str(),
    )
    .bind(&failure.mapping_id)
    .bind(&failure.table)
    .bind(failure.rendered_error())
    .execute(transaction.as_mut())
    .await
    .map_err(|source| RunnerReconcileRuntimeError::PersistFailureTrackingState {
        mapping_id: failure.mapping_id.clone(),
        database: failure.database.clone(),
        source,
    })?;

    if result.rows_affected() != 1 {
        return Err(RunnerReconcileRuntimeError::MissingTableTrackingState {
            mapping_id: failure.mapping_id,
            database: failure.database,
            table: failure.table,
        });
    }

    transaction
        .commit()
        .await
        .map_err(|source| RunnerReconcileRuntimeError::CommitFailureTrackingTransaction {
            mapping_id: failure.mapping_id,
            database: failure.database,
            source,
        })?;
    Ok(())
}
