mod delete;
mod upsert;

use std::{
    sync::Arc,
    time::{Instant, SystemTime},
};

use sqlx::{Connection, PgConnection};
use tokio::task::JoinSet;

use crate::{
    error::RunnerReconcileRuntimeError,
    runtime_plan::{MappingRuntimePlan, RunnerRuntimePlan},
    tracking_state::{
        ReconcileFailure, ReconcilePhase, persist_reconcile_failure, persist_reconcile_success,
    },
};

pub(crate) async fn serve(
    runtime: Arc<RunnerRuntimePlan>,
) -> Result<(), RunnerReconcileRuntimeError> {
    let mut workers = JoinSet::new();

    for destination_group in runtime.destination_groups() {
        for mapping in destination_group.mappings().iter().cloned() {
            let interval = runtime.reconcile_interval();
            let runtime = runtime.clone();
            workers.spawn(async move { run_mapping_loop(runtime, mapping, interval).await });
        }
    }

    match workers.join_next().await {
        Some(Ok(result)) => result,
        Some(Err(source)) => Err(RunnerReconcileRuntimeError::WorkerTask { source }),
        None => Ok(()),
    }
}

async fn run_mapping_loop(
    runtime: Arc<RunnerRuntimePlan>,
    mapping: MappingRuntimePlan,
    interval: std::time::Duration,
) -> Result<(), RunnerReconcileRuntimeError> {
    let mut ticker = tokio::time::interval_at(tokio::time::Instant::now() + interval, interval);

    loop {
        ticker.tick().await;
        let _pass_outcome = run_reconcile_pass(runtime.as_ref(), &mapping).await?;
    }
}

enum ReconcilePassOutcome {
    Succeeded,
    ApplyFailedRecorded,
}

async fn run_reconcile_pass(
    runtime: &RunnerRuntimePlan,
    mapping: &MappingRuntimePlan,
) -> Result<ReconcilePassOutcome, RunnerReconcileRuntimeError> {
    let endpoint = mapping.destination().endpoint_label();
    let database = mapping.destination().database().to_owned();
    let mut postgres = PgConnection::connect_with(&mapping.destination().connect_options())
            .await
            .map_err(|source| RunnerReconcileRuntimeError::Connect {
                mapping_id: mapping.mapping_id().to_owned(),
                endpoint,
                source,
            })?;
    let mut transaction =
        postgres
            .begin()
            .await
            .map_err(|source| RunnerReconcileRuntimeError::BeginTransaction {
                mapping_id: mapping.mapping_id().to_owned(),
                database: database.clone(),
                source,
            })?;

    match apply_reconcile_pass(runtime, &mut transaction, mapping).await {
        Ok(()) => {
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
            Ok(ReconcilePassOutcome::Succeeded)
        }
        Err(failure) => {
            let phase = failure.phase();
            let table = failure.table().to_owned();
            let error_detail = failure.error_detail();
            transaction.rollback().await.map_err(|source| {
                RunnerReconcileRuntimeError::Rollback {
                    mapping_id: mapping.mapping_id().to_owned(),
                    database: database.clone(),
                    source,
                }
            })?;
            persist_reconcile_failure(
                &mut postgres,
                ReconcileFailure::new(
                    mapping.mapping_id().to_owned(),
                    database.clone(),
                    table,
                    phase,
                    error_detail,
                ),
            )
            .await?;
            runtime.metrics().record_reconcile_apply_failure(
                mapping,
                failure.table(),
                phase,
                SystemTime::now(),
            );
            Ok(ReconcilePassOutcome::ApplyFailedRecorded)
        }
    }
}

async fn apply_reconcile_pass(
    runtime: &RunnerRuntimePlan,
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    mapping: &MappingRuntimePlan,
) -> Result<(), ReconcileApplyFailure> {
    for table in mapping.reconcile_upsert_tables() {
        let started_at = Instant::now();
        upsert::apply(transaction, mapping, table).await?;
        runtime.metrics().record_reconcile_apply(
            mapping,
            table,
            ReconcilePhase::Upsert,
            started_at.elapsed(),
            SystemTime::now(),
        );
    }
    for table in mapping.reconcile_delete_tables() {
        let started_at = Instant::now();
        delete::apply(transaction, mapping, table).await?;
        runtime.metrics().record_reconcile_apply(
            mapping,
            table,
            ReconcilePhase::Delete,
            started_at.elapsed(),
            SystemTime::now(),
        );
    }
    Ok(())
}

pub(super) enum ReconcileApplyFailure {
    MissingPrimaryKey {
        phase: ReconcilePhase,
        table: String,
    },
    Apply {
        phase: ReconcilePhase,
        table: String,
        source: String,
    },
}

impl ReconcileApplyFailure {
    fn missing_upsert_primary_key(table: String) -> Self {
        Self::MissingPrimaryKey {
            phase: ReconcilePhase::Upsert,
            table,
        }
    }

    fn missing_delete_primary_key(table: String) -> Self {
        Self::MissingPrimaryKey {
            phase: ReconcilePhase::Delete,
            table,
        }
    }

    fn apply_upsert(table: String, source: sqlx::Error) -> Self {
        Self::Apply {
            phase: ReconcilePhase::Upsert,
            table,
            source: source.to_string(),
        }
    }

    fn apply_delete(table: String, source: sqlx::Error) -> Self {
        Self::Apply {
            phase: ReconcilePhase::Delete,
            table,
            source: source.to_string(),
        }
    }

    fn phase(&self) -> ReconcilePhase {
        match self {
            Self::MissingPrimaryKey { phase, .. } | Self::Apply { phase, .. } => *phase,
        }
    }

    fn table(&self) -> &str {
        match self {
            Self::MissingPrimaryKey { table, .. } | Self::Apply { table, .. } => table,
        }
    }

    fn error_detail(&self) -> String {
        match self {
            Self::MissingPrimaryKey { .. } => "primary-key metadata is missing".to_owned(),
            Self::Apply { source, .. } => source.clone(),
        }
    }
}
