use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use crate::{
    config::{PostgresConnectionConfig, RunnerConfig},
    error::RunnerRuntimePlanError,
    helper_plan::{HelperShadowTablePlan, MappingHelperPlan},
};

pub(crate) struct RunnerRuntimePlan {
    bind_addr: std::net::SocketAddr,
    tls_cert_path: PathBuf,
    tls_key_path: PathBuf,
    reconcile_interval: Duration,
    mappings: BTreeMap<String, MappingRuntimePlan>,
}

impl RunnerRuntimePlan {
    pub(crate) fn from_config(
        config: &RunnerConfig,
        mut helper_plans: BTreeMap<String, MappingHelperPlan>,
    ) -> Result<Self, RunnerRuntimePlanError> {
        let mappings = config
            .mappings()
            .iter()
            .map(|mapping| {
                let helper_plan = helper_plans.remove(mapping.id()).ok_or_else(|| {
                    RunnerRuntimePlanError::MissingHelperPlan {
                        mapping_id: mapping.id().to_owned(),
                    }
                })?;
                Ok((
                    mapping.id().to_owned(),
                    MappingRuntimePlan::from_parts(
                        mapping.id(),
                        mapping.source().database(),
                        mapping.destination().connection().clone(),
                        helper_plan,
                    )?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, RunnerRuntimePlanError>>()?;

        Ok(Self {
            bind_addr: config.webhook().bind_addr(),
            tls_cert_path: config.webhook().tls().cert_path().to_path_buf(),
            tls_key_path: config.webhook().tls().key_path().to_path_buf(),
            reconcile_interval: Duration::from_secs(config.reconcile().interval_secs()),
            mappings,
        })
    }

    pub(crate) fn bind_addr(&self) -> std::net::SocketAddr {
        self.bind_addr
    }

    pub(crate) fn tls_cert_path(&self) -> &std::path::Path {
        &self.tls_cert_path
    }

    pub(crate) fn tls_key_path(&self) -> &std::path::Path {
        &self.tls_key_path
    }

    pub(crate) fn reconcile_interval(&self) -> Duration {
        self.reconcile_interval
    }

    pub(crate) fn require_mapping(
        &self,
        mapping_id: &str,
    ) -> Result<&MappingRuntimePlan, crate::error::RunnerWebhookRoutingError> {
        self.mappings
            .get(mapping_id)
            .ok_or_else(|| crate::error::RunnerWebhookRoutingError::UnknownMapping {
                mapping_id: mapping_id.to_owned(),
            })
    }

    pub(crate) fn mappings(&self) -> impl Iterator<Item = &MappingRuntimePlan> {
        self.mappings.values()
    }
}

#[derive(Clone)]
pub(crate) struct MappingRuntimePlan {
    mapping_id: String,
    source_database: String,
    destination_connection: PostgresConnectionConfig,
    helper_tables: BTreeMap<String, HelperShadowTablePlan>,
    reconcile_upsert_tables: Vec<HelperShadowTablePlan>,
    reconcile_delete_tables: Vec<HelperShadowTablePlan>,
}

impl MappingRuntimePlan {
    fn from_parts(
        mapping_id: &str,
        source_database: &str,
        destination_connection: PostgresConnectionConfig,
        helper_plan: MappingHelperPlan,
    ) -> Result<Self, RunnerRuntimePlanError> {
        let helper_tables = helper_plan
            .helper_tables()
            .iter()
            .cloned()
            .map(|table| (table.source_table().label(), table))
            .collect::<BTreeMap<_, _>>();
        let reconcile_upsert_tables =
            build_reconcile_tables(mapping_id, &helper_tables, helper_plan.reconcile_upsert_order())?;
        let reconcile_delete_tables =
            build_reconcile_tables(mapping_id, &helper_tables, helper_plan.reconcile_delete_order())?;

        Ok(Self {
            mapping_id: mapping_id.to_owned(),
            source_database: source_database.to_owned(),
            destination_connection,
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

    pub(crate) fn destination_connection(&self) -> &PostgresConnectionConfig {
        &self.destination_connection
    }

    pub(crate) fn helper_table(&self, table_label: &str) -> Option<&HelperShadowTablePlan> {
        self.helper_tables.get(table_label)
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
    helper_tables: &BTreeMap<String, HelperShadowTablePlan>,
    table_order: &[crate::sql_name::QualifiedTableName],
) -> Result<Vec<HelperShadowTablePlan>, RunnerRuntimePlanError> {
    table_order
        .iter()
        .map(|table_name| {
            helper_tables
                .get(&table_name.label())
                .cloned()
                .ok_or_else(|| RunnerRuntimePlanError::MissingReconcileTable {
                    mapping_id: mapping_id.to_owned(),
                    table: table_name.label(),
                })
        })
        .collect()
}
