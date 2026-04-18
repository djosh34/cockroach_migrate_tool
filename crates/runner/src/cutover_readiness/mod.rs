use sqlx::{Connection, PgConnection, Row};

use crate::{
    config::{LoadedRunnerConfig, MappingConfig},
    error::{RunnerCutoverReadinessError, RunnerVerifyError},
    molt_verify::run_verify,
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(crate) async fn run_cutover_readiness(
    loaded_config: &LoadedRunnerConfig,
    mapping_id: &str,
    source_url: &str,
    allow_tls_mode_disable: bool,
) -> Result<CutoverReadinessSummary, RunnerCutoverReadinessError> {
    let mapping = loaded_config.config().mapping(mapping_id).ok_or_else(|| {
        RunnerCutoverReadinessError::UnknownMapping {
            mapping_id: mapping_id.to_owned(),
            config_path: loaded_config.path().to_path_buf(),
        }
    })?;
    let request = CutoverReadinessRequest::from_mapping(mapping);
    let snapshot = request.read_snapshot().await?;
    let mut summary = CutoverReadinessSummary::from_snapshot(&request, snapshot)?;

    if summary.tables_drained && summary.watermarks_aligned {
        summary.verification = match run_verify(
            loaded_config,
            mapping_id,
            source_url,
            allow_tls_mode_disable,
        ) {
            Ok(_) => VerificationStatus::Matched,
            Err(RunnerVerifyError::DataMismatch { details, .. }) => {
                summary.ready = false;
                summary
                    .reasons
                    .push(format!("verification found mismatches: {details}"));
                VerificationStatus::Mismatch
            }
            Err(error) => return Err(RunnerCutoverReadinessError::Verify(error)),
        };
        if matches!(summary.verification, VerificationStatus::Matched) {
            summary.ready = true;
        }
    }

    Ok(summary)
}

struct CutoverReadinessRequest {
    mapping_id: String,
    database: String,
    endpoint: String,
    connection: crate::config::PostgresConnectionConfig,
    selected_tables: Vec<String>,
}

impl CutoverReadinessRequest {
    fn from_mapping(mapping: &MappingConfig) -> Self {
        let connection = mapping.destination().connection().clone();
        Self {
            mapping_id: mapping.id().to_owned(),
            database: connection.database().to_owned(),
            endpoint: connection.endpoint_label(),
            connection,
            selected_tables: mapping.source().tables().to_vec(),
        }
    }

    async fn read_snapshot(&self) -> Result<CutoverSnapshot, RunnerCutoverReadinessError> {
        let mut postgres = PgConnection::connect_with(&self.connection.connect_options())
            .await
            .map_err(|source| RunnerCutoverReadinessError::Connect {
                mapping_id: self.mapping_id.clone(),
                endpoint: self.endpoint.clone(),
                source,
            })?;
        let stream_row = sqlx::query(
            format!(
                "SELECT latest_received_resolved_watermark,
                        latest_reconciled_resolved_watermark
                 FROM {HELPER_SCHEMA}.stream_state
                 WHERE mapping_id = $1"
            )
            .as_str(),
        )
        .bind(&self.mapping_id)
        .fetch_optional(&mut postgres)
        .await
        .map_err(|source| RunnerCutoverReadinessError::ReadStreamState {
            mapping_id: self.mapping_id.clone(),
            database: self.database.clone(),
            source,
        })?
        .ok_or_else(|| RunnerCutoverReadinessError::MissingTrackingState {
            mapping_id: self.mapping_id.clone(),
            database: self.database.clone(),
        })?;

        let table_rows = sqlx::query(
            format!(
                "SELECT source_table_name,
                        last_successful_sync_watermark,
                        last_error
                 FROM {HELPER_SCHEMA}.table_sync_state
                 WHERE mapping_id = $1"
            )
            .as_str(),
        )
        .bind(&self.mapping_id)
        .fetch_all(&mut postgres)
        .await
        .map_err(|source| RunnerCutoverReadinessError::ReadTableSyncState {
            mapping_id: self.mapping_id.clone(),
            database: self.database.clone(),
            source,
        })?;

        postgres
            .close()
            .await
            .map_err(|source| RunnerCutoverReadinessError::Connect {
                mapping_id: self.mapping_id.clone(),
                endpoint: self.endpoint.clone(),
                source,
            })?;

        Ok(CutoverSnapshot {
            latest_received_resolved_watermark: stream_row
                .get::<Option<String>, _>("latest_received_resolved_watermark"),
            latest_reconciled_resolved_watermark: stream_row
                .get::<Option<String>, _>("latest_reconciled_resolved_watermark"),
            table_states: table_rows
                .into_iter()
                .map(|row| TableCutoverSnapshot {
                    source_table_name: row.get("source_table_name"),
                    last_successful_sync_watermark: row.get("last_successful_sync_watermark"),
                    last_error: row.get("last_error"),
                })
                .collect(),
        })
    }
}

struct CutoverSnapshot {
    latest_received_resolved_watermark: Option<String>,
    latest_reconciled_resolved_watermark: Option<String>,
    table_states: Vec<TableCutoverSnapshot>,
}

struct TableCutoverSnapshot {
    source_table_name: String,
    last_successful_sync_watermark: Option<String>,
    last_error: Option<String>,
}

pub struct CutoverReadinessSummary {
    mapping_id: String,
    ready: bool,
    latest_received_resolved_watermark: Option<String>,
    latest_reconciled_resolved_watermark: Option<String>,
    watermarks_aligned: bool,
    tables_drained: bool,
    verification: VerificationStatus,
    reasons: Vec<String>,
}

impl CutoverReadinessSummary {
    fn from_snapshot(
        request: &CutoverReadinessRequest,
        snapshot: CutoverSnapshot,
    ) -> Result<Self, RunnerCutoverReadinessError> {
        let mut reasons = Vec::new();
        let latest_received = snapshot.latest_received_resolved_watermark;
        let latest_reconciled = snapshot.latest_reconciled_resolved_watermark;
        let watermarks_aligned = match (latest_received.as_deref(), latest_reconciled.as_deref()) {
            (Some(received), Some(reconciled)) if received == reconciled => true,
            (Some(_), Some(_)) => {
                reasons.push(
                    "cdc/reconcile has not drained yet: received watermark is ahead of reconciled watermark"
                        .to_owned(),
                );
                false
            }
            _ => false,
        };

        if latest_received.is_none() {
            reasons.push("no received resolved watermark yet".to_owned());
        }
        if latest_reconciled.is_none() {
            reasons.push("no reconciled resolved watermark yet".to_owned());
        }

        let reconciled_watermark = latest_reconciled.clone();
        let mut tables_drained = true;
        for selected_table in &request.selected_tables {
            let row = snapshot
                .table_states
                .iter()
                .find(|row| &row.source_table_name == selected_table)
                .ok_or_else(|| RunnerCutoverReadinessError::MissingTableTrackingState {
                    mapping_id: request.mapping_id.clone(),
                    database: request.database.clone(),
                    table: selected_table.clone(),
                })?;
            if let Some(last_error) = row.last_error.as_deref() {
                tables_drained = false;
                reasons.push(format!(
                    "table drain is incomplete: {selected_table} has reconcile error: {last_error}"
                ));
            }
            if row.last_successful_sync_watermark != reconciled_watermark {
                tables_drained = false;
                reasons.push(format!(
                    "table drain is incomplete: {selected_table} has not drained to reconciled watermark"
                ));
            }
        }

        if matches!(
            (latest_received.as_deref(), latest_reconciled.as_deref()),
            (None, Some(_))
        ) {
            return Err(RunnerCutoverReadinessError::InvalidState {
                mapping_id: request.mapping_id.clone(),
                message: "reconciled watermark exists before a received watermark".to_owned(),
            });
        }

        Ok(Self {
            mapping_id: request.mapping_id.clone(),
            ready: false,
            latest_received_resolved_watermark: latest_received,
            latest_reconciled_resolved_watermark: latest_reconciled,
            watermarks_aligned,
            tables_drained,
            verification: VerificationStatus::Skipped,
            reasons,
        })
    }
}

impl std::fmt::Display for CutoverReadinessSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cutover readiness mapping={} ready={} received={} reconciled={} watermarks_aligned={} tables_drained={} verification={} reasons={}",
            self.mapping_id,
            self.ready,
            self.latest_received_resolved_watermark
                .as_deref()
                .unwrap_or("<none>"),
            self.latest_reconciled_resolved_watermark
                .as_deref()
                .unwrap_or("<none>"),
            self.watermarks_aligned,
            self.tables_drained,
            self.verification,
            if self.reasons.is_empty() {
                "none".to_owned()
            } else {
                self.reasons.join(" | ")
            }
        )
    }
}

enum VerificationStatus {
    Skipped,
    Matched,
    Mismatch,
}

impl std::fmt::Display for VerificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Skipped => write!(f, "skipped"),
            Self::Matched => write!(f, "matched"),
            Self::Mismatch => write!(f, "mismatch"),
        }
    }
}
