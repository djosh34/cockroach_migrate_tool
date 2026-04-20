use std::{
    collections::BTreeMap,
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::runtime_plan::MappingRuntimePlan;
use crate::{helper_plan::HelperShadowTablePlan, tracking_state::ReconcilePhase};

const METRIC_PREFIX: &str = "cockroach_migration_tool_";

pub(crate) struct RunnerMetrics {
    state: Mutex<RunnerMetricsState>,
}

impl RunnerMetrics {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(RunnerMetricsState::default()),
        }
    }

    pub(crate) fn record_webhook_request(
        &self,
        mapping: &MappingRuntimePlan,
        kind: WebhookKind,
        outcome: WebhookOutcome,
        recorded_at: SystemTime,
    ) {
        let mut state = self
            .state
            .lock()
            .expect("runner metrics mutex should not be poisoned");
        let timestamp = unix_timestamp_seconds(recorded_at);
        let destination_database = mapping.destination().database().to_owned();

        *state
            .webhook_requests_total
            .entry(WebhookRequestLabels {
                destination_database: destination_database.clone(),
                kind,
                outcome,
            })
            .or_default() += 1;

        state.webhook_last_request_unixtime_seconds.insert(
            WebhookLastRequestLabels {
                destination_database,
            },
            timestamp,
        );
    }

    pub(crate) fn render(&self) -> String {
        let state = self
            .state
            .lock()
            .expect("runner metrics mutex should not be poisoned");
        let mut lines = Vec::new();

        lines.push(format!(
            "# TYPE {METRIC_PREFIX}webhook_requests_total counter"
        ));
        for (labels, value) in &state.webhook_requests_total {
            lines.push(format!(
                "{METRIC_PREFIX}webhook_requests_total{} {}",
                labels.render(),
                value
            ));
        }

        lines.push(format!(
            "# TYPE {METRIC_PREFIX}webhook_last_request_unixtime_seconds gauge"
        ));
        for (labels, value) in &state.webhook_last_request_unixtime_seconds {
            lines.push(format!(
                "{METRIC_PREFIX}webhook_last_request_unixtime_seconds{} {}",
                labels.render(),
                format_metric_value(*value),
            ));
        }

        lines.push(format!(
            "# TYPE {METRIC_PREFIX}webhook_apply_duration_seconds_total counter"
        ));
        for (labels, value) in &state.webhook_apply_duration_seconds_total {
            lines.push(format!(
                "{METRIC_PREFIX}webhook_apply_duration_seconds_total{} {}",
                labels.render(),
                format_metric_value(*value),
            ));
        }

        lines.push(format!(
            "# TYPE {METRIC_PREFIX}webhook_apply_requests_total counter"
        ));
        for (labels, value) in &state.webhook_apply_requests_total {
            lines.push(format!(
                "{METRIC_PREFIX}webhook_apply_requests_total{} {}",
                labels.render(),
                value
            ));
        }

        lines.push(format!(
            "# TYPE {METRIC_PREFIX}webhook_apply_last_duration_seconds gauge"
        ));
        for (labels, value) in &state.webhook_apply_last_duration_seconds {
            lines.push(format!(
                "{METRIC_PREFIX}webhook_apply_last_duration_seconds{} {}",
                labels.render(),
                format_metric_value(*value),
            ));
        }

        lines.push(format!(
            "# TYPE {METRIC_PREFIX}reconcile_apply_duration_seconds_total counter"
        ));
        for (labels, value) in &state.reconcile_apply_duration_seconds_total {
            lines.push(format!(
                "{METRIC_PREFIX}reconcile_apply_duration_seconds_total{} {}",
                labels.render(),
                format_metric_value(*value),
            ));
        }

        lines.push(format!(
            "# TYPE {METRIC_PREFIX}reconcile_apply_attempts_total counter"
        ));
        for (labels, value) in &state.reconcile_apply_attempts_total {
            lines.push(format!(
                "{METRIC_PREFIX}reconcile_apply_attempts_total{} {}",
                labels.render(),
                value
            ));
        }

        lines.push(format!(
            "# TYPE {METRIC_PREFIX}reconcile_apply_last_duration_seconds gauge"
        ));
        for (labels, value) in &state.reconcile_apply_last_duration_seconds {
            lines.push(format!(
                "{METRIC_PREFIX}reconcile_apply_last_duration_seconds{} {}",
                labels.render(),
                format_metric_value(*value),
            ));
        }

        lines.push(format!(
            "# TYPE {METRIC_PREFIX}apply_failures_total counter"
        ));
        for (labels, value) in &state.apply_failures_total {
            lines.push(format!(
                "{METRIC_PREFIX}apply_failures_total{} {}",
                labels.render(),
                value
            ));
        }

        lines.push(format!(
            "# TYPE {METRIC_PREFIX}apply_last_outcome_unixtime_seconds gauge"
        ));
        for (labels, value) in &state.apply_last_outcome_unixtime_seconds {
            lines.push(format!(
                "{METRIC_PREFIX}apply_last_outcome_unixtime_seconds{} {}",
                labels.render(),
                format_metric_value(*value),
            ));
        }

        format!("{}\n", lines.join("\n"))
    }

    pub(crate) fn record_webhook_apply(
        &self,
        mapping: &MappingRuntimePlan,
        table: &HelperShadowTablePlan,
        duration: Duration,
        recorded_at: SystemTime,
    ) {
        let mut state = self
            .state
            .lock()
            .expect("runner metrics mutex should not be poisoned");
        let labels = TableMetricLabels {
            destination_database: mapping.destination().database().to_owned(),
            destination_table: table.source_table().label(),
        };
        let duration_seconds = duration.as_secs_f64();

        *state
            .webhook_apply_duration_seconds_total
            .entry(labels.clone())
            .or_default() += duration_seconds;
        *state
            .webhook_apply_requests_total
            .entry(labels.clone())
            .or_default() += 1;
        state
            .webhook_apply_last_duration_seconds
            .insert(labels, duration_seconds);
        record_apply_outcome(
            &mut state,
            mapping.destination().database(),
            table.source_table().label(),
            ApplyStage::WebhookApply,
            AttemptOutcome::Success,
            recorded_at,
        );
    }

    pub(crate) fn record_reconcile_apply(
        &self,
        mapping: &MappingRuntimePlan,
        table: &HelperShadowTablePlan,
        phase: ReconcilePhase,
        duration: Duration,
        recorded_at: SystemTime,
    ) {
        let mut state = self
            .state
            .lock()
            .expect("runner metrics mutex should not be poisoned");
        let labels = PhaseMetricLabels {
            destination_database: mapping.destination().database().to_owned(),
            destination_table: table.source_table().label(),
            phase,
        };
        let duration_seconds = duration.as_secs_f64();

        *state
            .reconcile_apply_duration_seconds_total
            .entry(labels.clone())
            .or_default() += duration_seconds;
        *state
            .reconcile_apply_attempts_total
            .entry(labels.clone())
            .or_default() += 1;
        state
            .reconcile_apply_last_duration_seconds
            .insert(labels, duration_seconds);
        record_apply_outcome(
            &mut state,
            mapping.destination().database(),
            table.source_table().label(),
            ApplyStage::for_reconcile_phase(phase),
            AttemptOutcome::Success,
            recorded_at,
        );
    }

    pub(crate) fn record_webhook_apply_failure(
        &self,
        mapping: &MappingRuntimePlan,
        table: &HelperShadowTablePlan,
        recorded_at: SystemTime,
    ) {
        self.record_apply_failure(
            mapping,
            table.source_table().label(),
            ApplyStage::WebhookApply,
            recorded_at,
        );
    }

    pub(crate) fn record_reconcile_apply_failure(
        &self,
        mapping: &MappingRuntimePlan,
        destination_table: &str,
        phase: ReconcilePhase,
        recorded_at: SystemTime,
    ) {
        self.record_apply_failure(
            mapping,
            destination_table.to_owned(),
            ApplyStage::for_reconcile_phase(phase),
            recorded_at,
        );
    }

    fn record_apply_failure(
        &self,
        mapping: &MappingRuntimePlan,
        destination_table: String,
        stage: ApplyStage,
        recorded_at: SystemTime,
    ) {
        let mut state = self
            .state
            .lock()
            .expect("runner metrics mutex should not be poisoned");
        let table_labels = TableStageMetricLabels {
            destination_database: mapping.destination().database().to_owned(),
            destination_table,
            stage,
        };

        record_apply_outcome(
            &mut state,
            &table_labels.destination_database,
            table_labels.destination_table.clone(),
            stage,
            AttemptOutcome::Error,
            recorded_at,
        );
        *state.apply_failures_total.entry(table_labels).or_default() += 1;
    }
}

#[derive(Default)]
struct RunnerMetricsState {
    webhook_requests_total: BTreeMap<WebhookRequestLabels, u64>,
    webhook_last_request_unixtime_seconds: BTreeMap<WebhookLastRequestLabels, f64>,
    webhook_apply_duration_seconds_total: BTreeMap<TableMetricLabels, f64>,
    webhook_apply_requests_total: BTreeMap<TableMetricLabels, u64>,
    webhook_apply_last_duration_seconds: BTreeMap<TableMetricLabels, f64>,
    reconcile_apply_duration_seconds_total: BTreeMap<PhaseMetricLabels, f64>,
    reconcile_apply_attempts_total: BTreeMap<PhaseMetricLabels, u64>,
    reconcile_apply_last_duration_seconds: BTreeMap<PhaseMetricLabels, f64>,
    apply_failures_total: BTreeMap<TableStageMetricLabels, u64>,
    apply_last_outcome_unixtime_seconds: BTreeMap<TableStageOutcomeMetricLabels, f64>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum WebhookKind {
    RowBatch,
    Resolved,
}

impl WebhookKind {
    fn as_label(self) -> &'static str {
        match self {
            Self::RowBatch => "row_batch",
            Self::Resolved => "resolved",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum WebhookOutcome {
    Ok,
    BadRequest,
    InternalError,
}

impl WebhookOutcome {
    fn as_label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::BadRequest => "bad_request",
            Self::InternalError => "internal_error",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct WebhookRequestLabels {
    destination_database: String,
    kind: WebhookKind,
    outcome: WebhookOutcome,
}

impl WebhookRequestLabels {
    fn render(&self) -> String {
        format!(
            "{{destination_database=\"{}\",kind=\"{}\",outcome=\"{}\"}}",
            self.destination_database,
            self.kind.as_label(),
            self.outcome.as_label(),
        )
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct WebhookLastRequestLabels {
    destination_database: String,
}

impl WebhookLastRequestLabels {
    fn render(&self) -> String {
        format!("{{destination_database=\"{}\"}}", self.destination_database,)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TableMetricLabels {
    destination_database: String,
    destination_table: String,
}

impl TableMetricLabels {
    fn render(&self) -> String {
        format!(
            "{{destination_database=\"{}\",destination_table=\"{}\"}}",
            self.destination_database, self.destination_table,
        )
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct PhaseMetricLabels {
    destination_database: String,
    destination_table: String,
    phase: ReconcilePhase,
}

impl PhaseMetricLabels {
    fn render(&self) -> String {
        format!(
            "{{destination_database=\"{}\",destination_table=\"{}\",phase=\"{}\"}}",
            self.destination_database, self.destination_table, self.phase,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ApplyStage {
    WebhookApply,
    ReconcileUpsert,
    ReconcileDelete,
}

impl ApplyStage {
    fn for_reconcile_phase(phase: ReconcilePhase) -> Self {
        match phase {
            ReconcilePhase::Upsert => Self::ReconcileUpsert,
            ReconcilePhase::Delete => Self::ReconcileDelete,
        }
    }

    fn as_label(self) -> &'static str {
        match self {
            Self::WebhookApply => "webhook_apply",
            Self::ReconcileUpsert => "reconcile_upsert",
            Self::ReconcileDelete => "reconcile_delete",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum AttemptOutcome {
    Success,
    Error,
}

impl AttemptOutcome {
    fn as_label(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TableStageMetricLabels {
    destination_database: String,
    destination_table: String,
    stage: ApplyStage,
}

impl TableStageMetricLabels {
    fn render(&self) -> String {
        format!(
            "{{destination_database=\"{}\",destination_table=\"{}\",stage=\"{}\"}}",
            self.destination_database,
            self.destination_table,
            self.stage.as_label(),
        )
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct TableStageOutcomeMetricLabels {
    destination_database: String,
    destination_table: String,
    stage: ApplyStage,
    outcome: AttemptOutcome,
}

impl TableStageOutcomeMetricLabels {
    fn render(&self) -> String {
        format!(
            "{{destination_database=\"{}\",destination_table=\"{}\",stage=\"{}\",outcome=\"{}\"}}",
            self.destination_database,
            self.destination_table,
            self.stage.as_label(),
            self.outcome.as_label(),
        )
    }
}

fn record_apply_outcome(
    state: &mut RunnerMetricsState,
    destination_database: &str,
    destination_table: String,
    stage: ApplyStage,
    outcome: AttemptOutcome,
    recorded_at: SystemTime,
) {
    state.apply_last_outcome_unixtime_seconds.insert(
        TableStageOutcomeMetricLabels {
            destination_database: destination_database.to_owned(),
            destination_table,
            stage,
            outcome,
        },
        unix_timestamp_seconds(recorded_at),
    );
}

fn unix_timestamp_seconds(timestamp: SystemTime) -> f64 {
    timestamp
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after the unix epoch")
        .as_secs_f64()
}

fn format_metric_value(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}
