use crate::{
    error::RunnerWebhookRoutingError,
    helper_plan::HelperShadowTablePlan,
    runtime_plan::MappingRuntimePlan,
    tracking_state::ResolvedTrackingTarget,
    webhook_runtime::{
        payload::{ResolvedRequest, RowBatchRequest, RowKey, RowMutation, WebhookRequest},
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
    DispatchTarget::Resolved(Box::new(ResolvedTrackingTarget {
        mapping_id: mapping.mapping_id().to_owned(),
        destination: mapping.destination().clone(),
        resolved_watermark: resolved.resolved().to_owned(),
    }))
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

        let table = source.source_table();
        let helper_table = mapping.helper_table(table).ok_or_else(|| {
            RunnerWebhookRoutingError::SourceTableNotMapped {
                mapping_id: mapping.mapping_id().to_owned(),
                table: table.label(),
            }
        })?;

        match &selected_table {
            Some(existing) if existing.source_table() != table => {
                return Err(RunnerWebhookRoutingError::MixedSourceTables {
                    mapping_id: mapping.mapping_id().to_owned(),
                    first: existing.source_table().label(),
                    second: table.label(),
                });
            }
            Some(_) => {}
            None => selected_table = Some(helper_table.clone()),
        }
    }

    let selected_table =
        selected_table.ok_or_else(|| RunnerWebhookRoutingError::EmptyRowBatch {
            mapping_id: mapping.mapping_id().to_owned(),
        })?;
    let rows = batch
        .into_rows()
        .into_iter()
        .map(|row| normalize_row_mutation(mapping, &selected_table, row.into_mutation()))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(DispatchTarget::RowBatch(Box::new(RowMutationBatch {
        mapping_id: mapping.mapping_id().to_owned(),
        destination: mapping.destination().clone(),
        table: selected_table,
        rows,
    })))
}

fn normalize_row_mutation(
    mapping: &MappingRuntimePlan,
    table: &HelperShadowTablePlan,
    mutation: RowMutation,
) -> Result<RowMutation, RunnerWebhookRoutingError> {
    let (operation, key, values) = mutation.into_raw_parts();
    let key = match key {
        RowKey::Named(key) => key,
        RowKey::Positional(values) => {
            if values.len() != table.primary_key_columns().len() {
                return Err(RunnerWebhookRoutingError::InvalidPrimaryKeyCount {
                    mapping_id: mapping.mapping_id().to_owned(),
                    table: table.source_table().label(),
                    expected: table.primary_key_columns().len(),
                    actual: values.len(),
                });
            }

            table
                .primary_key_columns()
                .iter()
                .zip(values)
                .map(|(column, value)| (column.raw().to_owned(), value))
                .collect()
        }
    };

    Ok(RowMutation::from_parts(operation, key, values))
}

pub(crate) enum DispatchTarget {
    RowBatch(Box<RowMutationBatch>),
    Resolved(Box<ResolvedTrackingTarget>),
}
