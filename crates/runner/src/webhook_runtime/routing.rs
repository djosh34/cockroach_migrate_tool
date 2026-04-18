use crate::{
    error::RunnerWebhookRoutingError,
    helper_plan::HelperShadowTablePlan,
    runtime_plan::MappingRuntimePlan,
    tracking_state::ResolvedTrackingTarget,
    webhook_runtime::{
        payload::{ResolvedRequest, RowBatchRequest, WebhookRequest},
        persistence::RowMutationBatch,
    },
};

pub(crate) fn route_request(
    mapping: &MappingRuntimePlan,
    request: WebhookRequest,
) -> Result<DispatchTarget, RunnerWebhookRoutingError> {
    match request {
        WebhookRequest::Resolved(resolved) => Ok(route_resolved(mapping, resolved)),
        WebhookRequest::RowBatch(batch) => route_row_batch(mapping, batch),
    }
}

fn route_resolved(mapping: &MappingRuntimePlan, resolved: ResolvedRequest) -> DispatchTarget {
    DispatchTarget::Resolved(ResolvedTrackingTarget {
        mapping_id: mapping.mapping_id().to_owned(),
        connection: mapping.destination_connection().clone(),
        resolved_watermark: resolved.resolved().to_owned(),
    })
}

fn route_row_batch(
    mapping: &MappingRuntimePlan,
    batch: RowBatchRequest,
) -> Result<DispatchTarget, RunnerWebhookRoutingError> {
    let mut selected_table = None::<HelperShadowTablePlan>;
    for row in batch.rows() {
        let source = row.source();
        if source.database_name() != mapping.source_database() {
            return Err(RunnerWebhookRoutingError::SourceDatabaseMismatch {
                mapping_id: mapping.mapping_id().to_owned(),
                expected: mapping.source_database().to_owned(),
                database: source.database_name().to_owned(),
            });
        }

        let table = source.table_label();
        let helper_table =
            mapping
                .helper_table(&table)
                .ok_or_else(|| RunnerWebhookRoutingError::SourceTableNotMapped {
                    mapping_id: mapping.mapping_id().to_owned(),
                    table: table.clone(),
                })?;

        match &selected_table {
            Some(existing) if existing.source_table().label() != table => {
                return Err(RunnerWebhookRoutingError::MixedSourceTables {
                    mapping_id: mapping.mapping_id().to_owned(),
                    first: existing.source_table().label(),
                    second: table,
                });
            }
            Some(_) => {}
            None => selected_table = Some(helper_table.clone()),
        }
    }

    let selected_table = selected_table.ok_or_else(|| RunnerWebhookRoutingError::EmptyRowBatch {
        mapping_id: mapping.mapping_id().to_owned(),
    })?;

    Ok(DispatchTarget::RowBatch(Box::new(RowMutationBatch {
        mapping_id: mapping.mapping_id().to_owned(),
        connection: mapping.destination_connection().clone(),
        table: selected_table,
        rows: batch
            .into_rows()
            .into_iter()
            .map(|row| row.into_mutation())
            .collect(),
    })))
}

pub(crate) enum DispatchTarget {
    RowBatch(Box<RowMutationBatch>),
    Resolved(ResolvedTrackingTarget),
}
