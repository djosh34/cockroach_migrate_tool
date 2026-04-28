use runner_config::{QualifiedTableName, SqlIdentifier};
use serde_json::{Map, Value};

use crate::error::RunnerWebhookPayloadError;
use crate::metrics::WebhookKind;

#[derive(Clone, Debug)]
pub(crate) enum WebhookRequest {
    RowBatch(RowBatchRequest),
    Resolved(ResolvedRequest),
}

impl WebhookRequest {
    pub(crate) fn kind(&self) -> WebhookKind {
        match self {
            Self::RowBatch(_) => WebhookKind::RowBatch,
            Self::Resolved(_) => WebhookKind::Resolved,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RowBatchRequest {
    rows: Vec<RowEvent>,
}

impl RowBatchRequest {
    pub(crate) fn rows(&self) -> &[RowEvent] {
        &self.rows
    }

    pub(crate) fn into_rows(self) -> Vec<RowEvent> {
        self.rows
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RowEvent {
    source: SourceMetadata,
    mutation: RowMutation,
}

impl RowEvent {
    pub(crate) fn source(&self) -> &SourceMetadata {
        &self.source
    }

    pub(crate) fn into_mutation(self) -> RowMutation {
        self.mutation
    }
}

#[derive(Clone, Debug)]
pub(crate) struct RowMutation {
    operation: RowOperation,
    key: Map<String, Value>,
    values: Option<Map<String, Value>>,
}

impl RowMutation {
    pub(crate) fn operation(&self) -> RowOperation {
        self.operation
    }

    pub(crate) fn key(&self) -> &Map<String, Value> {
        &self.key
    }

    pub(crate) fn values(&self) -> Option<&Map<String, Value>> {
        self.values.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RowOperation {
    Upsert,
    Delete,
}

#[derive(Clone, Debug)]
pub(crate) struct SourceMetadata {
    database_name: String,
    source_table: QualifiedTableName,
}

impl SourceMetadata {
    pub(crate) fn database_name(&self) -> &str {
        &self.database_name
    }

    pub(crate) fn source_table(&self) -> &QualifiedTableName {
        &self.source_table
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedRequest {
    resolved: String,
}

impl ResolvedRequest {
    pub(crate) fn resolved(&self) -> &str {
        &self.resolved
    }
}

pub(crate) fn parse_webhook_request(
    body: &[u8],
) -> Result<WebhookRequest, RunnerWebhookPayloadError> {
    let value = serde_json::from_slice::<Value>(body)
        .map_err(|source| RunnerWebhookPayloadError::InvalidJson { source })?;
    let object = value
        .as_object()
        .ok_or(RunnerWebhookPayloadError::ExpectedObject)?;

    if let Some(resolved) = object.get("resolved") {
        let resolved = resolved
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or(RunnerWebhookPayloadError::InvalidResolved)?;
        return Ok(WebhookRequest::Resolved(ResolvedRequest {
            resolved: resolved.to_owned(),
        }));
    }

    let Some(payload) = object.get("payload") else {
        return Err(RunnerWebhookPayloadError::UnsupportedShape);
    };
    let payload = payload
        .as_array()
        .ok_or(RunnerWebhookPayloadError::MissingPayload)?;
    let length = object
        .get("length")
        .and_then(Value::as_u64)
        .ok_or(RunnerWebhookPayloadError::MissingLength)?;
    if length as usize != payload.len() {
        return Err(RunnerWebhookPayloadError::LengthMismatch);
    }
    if payload.is_empty() {
        return Err(RunnerWebhookPayloadError::EmptyPayload);
    }

    let mut rows = Vec::with_capacity(payload.len());
    for row in payload {
        rows.push(parse_row_event(row.clone())?);
    }
    Ok(WebhookRequest::RowBatch(RowBatchRequest { rows }))
}

fn parse_row_event(value: Value) -> Result<RowEvent, RunnerWebhookPayloadError> {
    let object = value
        .as_object()
        .ok_or(RunnerWebhookPayloadError::InvalidRowEvent)?;
    let source = object
        .get("source")
        .ok_or(RunnerWebhookPayloadError::MissingSource)
        .and_then(parse_source_metadata)?;
    Ok(RowEvent {
        source,
        mutation: parse_row_mutation(object)?,
    })
}

fn parse_row_mutation(
    object: &serde_json::Map<String, Value>,
) -> Result<RowMutation, RunnerWebhookPayloadError> {
    let operation = parse_row_operation(object)?;
    let key = parse_object_field(
        object,
        "key",
        RunnerWebhookPayloadError::MissingKey,
        RunnerWebhookPayloadError::InvalidKey,
    )?;
    let values = match operation {
        RowOperation::Upsert => Some(parse_object_field(
            object,
            "after",
            RunnerWebhookPayloadError::MissingAfter,
            RunnerWebhookPayloadError::InvalidAfter,
        )?),
        RowOperation::Delete => None,
    };

    Ok(RowMutation {
        operation,
        key,
        values,
    })
}

fn parse_row_operation(
    object: &serde_json::Map<String, Value>,
) -> Result<RowOperation, RunnerWebhookPayloadError> {
    let op = object
        .get("op")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or(RunnerWebhookPayloadError::MissingOperation)?;

    match op {
        "c" | "u" | "r" => Ok(RowOperation::Upsert),
        "d" => Ok(RowOperation::Delete),
        other => Err(RunnerWebhookPayloadError::UnsupportedOperation {
            op: other.to_owned(),
        }),
    }
}

fn parse_source_metadata(value: &Value) -> Result<SourceMetadata, RunnerWebhookPayloadError> {
    let object = value
        .as_object()
        .ok_or(RunnerWebhookPayloadError::InvalidSource)?;
    Ok(SourceMetadata {
        database_name: required_string_field(object, "database_name")?,
        source_table: QualifiedTableName::new(
            SqlIdentifier::new(&required_string_field(object, "schema_name")?),
            SqlIdentifier::new(&required_string_field(object, "table_name")?),
        ),
    })
}

fn required_string_field(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
) -> Result<String, RunnerWebhookPayloadError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or(RunnerWebhookPayloadError::MissingSourceField { field })
}

fn parse_object_field(
    object: &serde_json::Map<String, Value>,
    field: &'static str,
    missing_error: RunnerWebhookPayloadError,
    invalid_error: RunnerWebhookPayloadError,
) -> Result<Map<String, Value>, RunnerWebhookPayloadError> {
    object
        .get(field)
        .ok_or(missing_error)?
        .as_object()
        .cloned()
        .ok_or(invalid_error)
}
