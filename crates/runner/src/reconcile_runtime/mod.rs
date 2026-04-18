mod delete;
mod upsert;

use std::sync::Arc;

use sqlx::{Connection, PgConnection};
use tokio::task::JoinSet;

use crate::{
    error::RunnerReconcileRuntimeError,
    runtime_plan::{MappingRuntimePlan, RunnerRuntimePlan},
    webhook_runtime::tracking::persist_reconcile_success,
};

pub(crate) async fn serve(
    runtime: Arc<RunnerRuntimePlan>,
) -> Result<(), RunnerReconcileRuntimeError> {
    let mut workers = JoinSet::new();

    for mapping in runtime.mappings().cloned() {
        let interval = runtime.reconcile_interval();
        workers.spawn(async move { run_mapping_loop(mapping, interval).await });
    }

    match workers.join_next().await {
        Some(Ok(result)) => result,
        Some(Err(source)) => Err(RunnerReconcileRuntimeError::WorkerTask { source }),
        None => Ok(()),
    }
}

async fn run_mapping_loop(
    mapping: MappingRuntimePlan,
    interval: std::time::Duration,
) -> Result<(), RunnerReconcileRuntimeError> {
    let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + interval, interval);

    loop {
        ticker.tick().await;
        run_reconcile_pass(&mapping).await?;
    }
}

async fn run_reconcile_pass(
    mapping: &MappingRuntimePlan,
) -> Result<(), RunnerReconcileRuntimeError> {
    let endpoint = mapping.destination_connection().endpoint_label();
    let database = mapping.destination_connection().database().to_owned();
    let mut postgres = PgConnection::connect_with(&mapping.destination_connection().connect_options())
        .await
        .map_err(|source| RunnerReconcileRuntimeError::Connect {
            mapping_id: mapping.mapping_id().to_owned(),
            endpoint,
            source,
        })?;
    let mut transaction = postgres
        .begin()
        .await
        .map_err(|source| RunnerReconcileRuntimeError::BeginTransaction {
            mapping_id: mapping.mapping_id().to_owned(),
            database: database.clone(),
            source,
        })?;

    for table in mapping.reconcile_upsert_tables() {
        upsert::apply(&mut transaction, mapping, table).await?;
    }
    for table in mapping.reconcile_delete_tables() {
        delete::apply(&mut transaction, mapping, table).await?;
    }
    persist_reconcile_success(
        &mut transaction,
        mapping.mapping_id(),
        &database,
        mapping.reconcile_upsert_tables(),
    )
    .await?;

    transaction
        .commit()
        .await
        .map_err(|source| RunnerReconcileRuntimeError::Commit {
            mapping_id: mapping.mapping_id().to_owned(),
            database,
            source,
        })?;
    Ok(())
}
