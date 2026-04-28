use runner_config::{PostgresTargetConfig, SqlIdentifier};
use sqlx::{Connection, PgConnection, Postgres, Transaction, types::Json};

use crate::{
    error::RunnerWebhookPersistenceError,
    helper_plan::HelperShadowTablePlan,
    webhook_runtime::payload::{RowMutation, RowOperation},
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(crate) struct RowMutationBatch {
    pub(crate) mapping_id: String,
    pub(crate) destination: PostgresTargetConfig,
    pub(crate) table: HelperShadowTablePlan,
    pub(crate) rows: Vec<RowMutation>,
}

pub(crate) async fn persist_row_batch(
    batch: RowMutationBatch,
) -> Result<(), RunnerWebhookPersistenceError> {
    let endpoint = batch.destination.endpoint_label();
    let database = batch.destination.database().to_owned();
    let mut postgres = PgConnection::connect_with(&batch.destination.connect_options())
        .await
        .map_err(|source| RunnerWebhookPersistenceError::Connect {
            mapping_id: batch.mapping_id.clone(),
            endpoint,
            source,
        })?;
    let mut transaction = postgres.begin().await.map_err(|source| {
        RunnerWebhookPersistenceError::BeginTransaction {
            mapping_id: batch.mapping_id.clone(),
            database: database.clone(),
            source,
        }
    })?;

    for row in &batch.rows {
        match row.operation() {
            RowOperation::Upsert => persist_upsert(&mut transaction, &batch, row).await?,
            RowOperation::Delete => persist_delete(&mut transaction, &batch, row).await?,
        }
    }

    transaction
        .commit()
        .await
        .map_err(|source| RunnerWebhookPersistenceError::Commit {
            mapping_id: batch.mapping_id,
            database,
            source,
        })?;
    Ok(())
}

async fn persist_upsert(
    transaction: &mut Transaction<'_, Postgres>,
    batch: &RowMutationBatch,
    row: &RowMutation,
) -> Result<(), RunnerWebhookPersistenceError> {
    let values = serde_json::Value::Object(row.values().cloned().ok_or_else(|| {
        RunnerWebhookPersistenceError::MissingValues {
            mapping_id: batch.mapping_id.clone(),
            helper_table: batch.table.helper_table_name().to_owned(),
        }
    })?);
    sqlx::query(&render_upsert_sql(&batch.table))
        .bind(Json(values))
        .execute(transaction.as_mut())
        .await
        .map(|_| ())
        .map_err(|source| RunnerWebhookPersistenceError::ApplyMutation {
            mapping_id: batch.mapping_id.clone(),
            helper_table: batch.table.helper_table_name().to_owned(),
            source,
        })
}

fn render_upsert_sql(table: &HelperShadowTablePlan) -> String {
    let helper_table = render_helper_table_name(table);
    let columns = table
        .columns()
        .iter()
        .map(|column| column.name().to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let selected_columns = table
        .columns()
        .iter()
        .map(|column| format!("row_data.{}", column.name()))
        .collect::<Vec<_>>()
        .join(", ");

    if table.primary_key_columns().is_empty() {
        return format!(
            "INSERT INTO {helper_table} ({columns}) \
             SELECT {selected_columns} \
             FROM jsonb_populate_record(NULL::{helper_table}, $1::jsonb) AS row_data"
        );
    }

    let conflict_target = table
        .primary_key_columns()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let assignments = table
        .columns()
        .iter()
        .map(|column| format!("{0} = EXCLUDED.{0}", column.name()))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "INSERT INTO {helper_table} ({columns}) \
         SELECT {selected_columns} \
         FROM jsonb_populate_record(NULL::{helper_table}, $1::jsonb) AS row_data \
         ON CONFLICT ({conflict_target}) DO UPDATE SET {assignments}"
    )
}

async fn persist_delete(
    transaction: &mut Transaction<'_, Postgres>,
    batch: &RowMutationBatch,
    row: &RowMutation,
) -> Result<(), RunnerWebhookPersistenceError> {
    if batch.table.primary_key_columns().is_empty() {
        return Err(RunnerWebhookPersistenceError::MissingPrimaryKey {
            mapping_id: batch.mapping_id.clone(),
            helper_table: batch.table.helper_table_name().to_owned(),
        });
    }

    let key = serde_json::Value::Object(row.key().clone());
    sqlx::query(&render_delete_sql(&batch.table))
        .bind(Json(key))
        .execute(transaction.as_mut())
        .await
        .map(|_| ())
        .map_err(|source| RunnerWebhookPersistenceError::ApplyMutation {
            mapping_id: batch.mapping_id.clone(),
            helper_table: batch.table.helper_table_name().to_owned(),
            source,
        })
}

fn render_delete_sql(table: &HelperShadowTablePlan) -> String {
    let helper_table = render_helper_table_name(table);
    let predicates = table
        .primary_key_columns()
        .iter()
        .map(|column| format!("target.{0} IS NOT DISTINCT FROM key_data.{0}", column))
        .collect::<Vec<_>>()
        .join(" AND ");

    format!(
        "DELETE FROM {helper_table} AS target \
         USING jsonb_populate_record(NULL::{helper_table}, $1::jsonb) AS key_data \
         WHERE {predicates}"
    )
}

fn render_helper_table_name(table: &HelperShadowTablePlan) -> String {
    format!(
        "{}.{}",
        SqlIdentifier::new(HELPER_SCHEMA),
        SqlIdentifier::new(table.helper_table_name())
    )
}
