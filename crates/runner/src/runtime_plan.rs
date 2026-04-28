use std::collections::BTreeMap;

use runner_config::{
    ConfiguredMappingPlan, PostgresTargetConfig, QualifiedTableName, RunnerStartupPlan,
    WebhookListenerPlan,
};

use crate::{
    error::{RunnerRuntimePlanError, RunnerWebhookRoutingError},
    helper_plan::{HelperShadowTablePlan, MappingHelperPlan},
    metrics::RunnerMetrics,
};

pub(crate) struct RunnerRuntimePlan {
    webhook_listener: WebhookListenerPlan,
    reconcile_interval: std::time::Duration,
    metrics: RunnerMetrics,
    mappings: BTreeMap<String, MappingRuntimePlan>,
    destination_groups: Vec<DestinationRuntimePlan>,
}

impl RunnerRuntimePlan {
    pub(crate) fn from_startup_plan(
        startup_plan: RunnerStartupPlan,
        mut helper_plans: BTreeMap<String, MappingHelperPlan>,
    ) -> Result<Self, RunnerRuntimePlanError> {
        let mut mappings = BTreeMap::new();

        for mapping in startup_plan.mappings().values() {
            let helper_plan = helper_plans.remove(mapping.mapping_id()).ok_or_else(|| {
                RunnerRuntimePlanError::MissingHelperPlan {
                    mapping_id: mapping.mapping_id().to_owned(),
                }
            })?;
            mappings.insert(
                mapping.mapping_id().to_owned(),
                MappingRuntimePlan::from_parts(mapping, helper_plan)?,
            );
        }

        let destination_groups = startup_plan
            .destination_groups()
            .iter()
            .map(|destination_group| {
                let mappings = destination_group
                    .mappings()
                    .iter()
                    .map(|mapping| {
                        mappings.get(mapping.mapping_id()).cloned().unwrap_or_else(|| {
                            panic!(
                                "runtime mappings should exist for destination group mapping `{}`",
                                mapping.mapping_id()
                            )
                        })
                    })
                    .collect();
                DestinationRuntimePlan { mappings }
            })
            .collect();

        Ok(Self {
            webhook_listener: startup_plan.webhook_listener().clone(),
            reconcile_interval: startup_plan.reconcile_interval(),
            metrics: RunnerMetrics::new(),
            mappings,
            destination_groups,
        })
    }

    pub(crate) fn webhook_listener(&self) -> &WebhookListenerPlan {
        &self.webhook_listener
    }

    pub(crate) fn bind_addr(&self) -> std::net::SocketAddr {
        self.webhook_listener.bind_addr()
    }

    pub(crate) fn reconcile_interval(&self) -> std::time::Duration {
        self.reconcile_interval
    }

    pub(crate) fn metrics(&self) -> &RunnerMetrics {
        &self.metrics
    }

    pub(crate) fn require_mapping(
        &self,
        mapping_id: &str,
    ) -> Result<&MappingRuntimePlan, RunnerWebhookRoutingError> {
        self.mappings
            .get(mapping_id)
            .ok_or_else(|| RunnerWebhookRoutingError::UnknownMapping {
                mapping_id: mapping_id.to_owned(),
            })
    }

    pub(crate) fn destination_groups(&self) -> &[DestinationRuntimePlan] {
        &self.destination_groups
    }
}

#[derive(Clone)]
pub(crate) struct DestinationRuntimePlan {
    mappings: Vec<MappingRuntimePlan>,
}

impl DestinationRuntimePlan {
    pub(crate) fn mappings(&self) -> &[MappingRuntimePlan] {
        &self.mappings
    }
}

#[derive(Clone)]
pub(crate) struct MappingRuntimePlan {
    mapping_id: String,
    source_database: String,
    destination: PostgresTargetConfig,
    helper_tables: BTreeMap<QualifiedTableName, HelperShadowTablePlan>,
    reconcile_upsert_tables: Vec<HelperShadowTablePlan>,
    reconcile_delete_tables: Vec<HelperShadowTablePlan>,
}

impl MappingRuntimePlan {
    fn from_parts(
        mapping: &ConfiguredMappingPlan,
        helper_plan: MappingHelperPlan,
    ) -> Result<Self, RunnerRuntimePlanError> {
        let helper_tables = helper_plan
            .helper_tables()
            .iter()
            .cloned()
            .map(|table| (table.source_table().clone(), table))
            .collect::<BTreeMap<_, _>>();
        let reconcile_upsert_tables = build_reconcile_tables(
            mapping.mapping_id(),
            &helper_tables,
            helper_plan.reconcile_upsert_order(),
        )?;
        let reconcile_delete_tables = build_reconcile_tables(
            mapping.mapping_id(),
            &helper_tables,
            helper_plan.reconcile_delete_order(),
        )?;

        Ok(Self {
            mapping_id: mapping.mapping_id().to_owned(),
            source_database: mapping.source_database().to_owned(),
            destination: mapping.destination().clone(),
            helper_tables,
            reconcile_upsert_tables,
            reconcile_delete_tables,
        })
    }

    pub(crate) fn mapping_id(&self) -> &str {
        &self.mapping_id
    }

    pub(crate) fn source_database(&self) -> &str {
        &self.source_database
    }

    pub(crate) fn destination(&self) -> &PostgresTargetConfig {
        &self.destination
    }

    pub(crate) fn helper_table(
        &self,
        table_name: &QualifiedTableName,
    ) -> Option<&HelperShadowTablePlan> {
        self.helper_tables.get(table_name)
    }

    pub(crate) fn reconcile_upsert_tables(&self) -> &[HelperShadowTablePlan] {
        &self.reconcile_upsert_tables
    }

    pub(crate) fn reconcile_delete_tables(&self) -> &[HelperShadowTablePlan] {
        &self.reconcile_delete_tables
    }
}

fn build_reconcile_tables(
    mapping_id: &str,
    helper_tables: &BTreeMap<QualifiedTableName, HelperShadowTablePlan>,
    table_order: &[QualifiedTableName],
) -> Result<Vec<HelperShadowTablePlan>, RunnerRuntimePlanError> {
    table_order
        .iter()
        .map(|table_name| {
            helper_tables.get(table_name).cloned().ok_or_else(|| {
                RunnerRuntimePlanError::MissingReconcileTable {
                    mapping_id: mapping_id.to_owned(),
                    table: table_name.label(),
                }
            })
        })
        .collect()
}
