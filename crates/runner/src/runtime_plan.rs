use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use crate::{
    config::{MappingConfig, PostgresTargetConfig, RunnerConfig, WebhookConfig},
    error::{RunnerRuntimePlanError, RunnerWebhookRoutingError},
    helper_plan::{HelperShadowTablePlan, MappingHelperPlan},
    metrics::RunnerMetrics,
    sql_name::QualifiedTableName,
};

pub(crate) struct RunnerStartupPlan {
    webhook_listener: WebhookListenerPlan,
    reconcile_interval: Duration,
    mappings: BTreeMap<String, ConfiguredMappingPlan>,
    destination_groups: Vec<DestinationGroupPlan>,
}

impl RunnerStartupPlan {
    pub(crate) fn from_config(config: &RunnerConfig) -> Result<Self, RunnerRuntimePlanError> {
        let mut mappings = BTreeMap::new();
        let mut grouped_mappings =
            BTreeMap::<DestinationDatabaseKey, Vec<ConfiguredMappingPlan>>::new();

        for mapping in config.mappings() {
            let mapping_plan = ConfiguredMappingPlan::from_config(mapping);
            grouped_mappings
                .entry(DestinationDatabaseKey::from_target(
                    mapping_plan.destination(),
                ))
                .or_default()
                .push(mapping_plan.clone());
            mappings.insert(mapping_plan.mapping_id().to_owned(), mapping_plan);
        }

        let destination_groups = grouped_mappings
            .into_iter()
            .map(|(database_key, mappings)| DestinationGroupPlan::new(database_key, mappings))
            .collect::<Result<Vec<_>, RunnerRuntimePlanError>>()?;

        Ok(Self {
            webhook_listener: WebhookListenerPlan::from_config(config.webhook()),
            reconcile_interval: Duration::from_secs(config.reconcile().interval_secs()),
            mappings,
            destination_groups,
        })
    }

    pub(crate) fn destination_groups(&self) -> &[DestinationGroupPlan] {
        &self.destination_groups
    }
}

#[derive(Clone)]
pub(crate) struct ConfiguredMappingPlan {
    mapping_id: String,
    source_database: String,
    destination: PostgresTargetConfig,
    selected_tables: Vec<QualifiedTableName>,
}

impl ConfiguredMappingPlan {
    fn from_config(mapping: &MappingConfig) -> Self {
        Self {
            mapping_id: mapping.id().to_owned(),
            source_database: mapping.source().database().to_owned(),
            destination: mapping.destination().clone(),
            selected_tables: mapping
                .source()
                .tables()
                .iter()
                .map(|table| QualifiedTableName::from_config(table))
                .collect(),
        }
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

    pub(crate) fn selected_tables(&self) -> &[QualifiedTableName] {
        &self.selected_tables
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct DestinationDatabaseKey {
    host: String,
    port: u16,
    database: String,
}

impl DestinationDatabaseKey {
    fn from_target(target: &PostgresTargetConfig) -> Self {
        Self {
            host: target.host().to_owned(),
            port: target.port(),
            database: target.database().to_owned(),
        }
    }

    pub(crate) fn label(&self) -> String {
        format!("{}:{}/{}", self.host, self.port, self.database)
    }
}

#[derive(Clone)]
pub(crate) struct DestinationGroupPlan {
    target: PostgresTargetConfig,
    mappings: Vec<ConfiguredMappingPlan>,
}

impl DestinationGroupPlan {
    fn new(
        database_key: DestinationDatabaseKey,
        mappings: Vec<ConfiguredMappingPlan>,
    ) -> Result<Self, RunnerRuntimePlanError> {
        let Some(target) = mappings
            .first()
            .map(|mapping| mapping.destination().clone())
        else {
            panic!("destination group should contain at least one mapping");
        };

        for mapping in &mappings {
            if !mapping.destination().same_target_contract(&target) {
                return Err(RunnerRuntimePlanError::InconsistentDestinationTarget {
                    destination: database_key.label(),
                    first_mapping_id: mappings
                        .first()
                        .map(|first| first.mapping_id().to_owned())
                        .unwrap_or_else(|| {
                            panic!("destination group should contain at least one mapping")
                        }),
                    second_mapping_id: mapping.mapping_id().to_owned(),
                });
            }
        }

        let mut table_owners = BTreeMap::<String, String>::new();
        for mapping in &mappings {
            for table in mapping.selected_tables() {
                let table = table.label();
                if let Some(first_mapping_id) =
                    table_owners.insert(table.clone(), mapping.mapping_id().to_owned())
                {
                    return Err(RunnerRuntimePlanError::OverlappingDestinationTable {
                        destination: database_key.label(),
                        table,
                        first_mapping_id,
                        second_mapping_id: mapping.mapping_id().to_owned(),
                    });
                }
            }
        }

        Ok(Self { target, mappings })
    }

    pub(crate) fn target(&self) -> &PostgresTargetConfig {
        &self.target
    }

    pub(crate) fn mappings(&self) -> &[ConfiguredMappingPlan] {
        &self.mappings
    }
}

pub(crate) struct RunnerRuntimePlan {
    webhook_listener: WebhookListenerPlan,
    reconcile_interval: Duration,
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

        for mapping in startup_plan.mappings.values() {
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
            .destination_groups
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
            webhook_listener: startup_plan.webhook_listener,
            reconcile_interval: startup_plan.reconcile_interval,
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

    pub(crate) fn reconcile_interval(&self) -> Duration {
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

pub(crate) struct WebhookListenerPlan {
    bind_addr: std::net::SocketAddr,
    transport: WebhookListenerTransport,
}

impl WebhookListenerPlan {
    fn from_config(config: &WebhookConfig) -> Self {
        Self {
            bind_addr: config.bind_addr(),
            transport: match config.tls() {
                Some(tls) => WebhookListenerTransport::Https {
                    cert_path: tls.cert_path().to_path_buf(),
                    key_path: tls.key_path().to_path_buf(),
                    client_ca_path: tls.client_ca_path().map(ToOwned::to_owned),
                },
                None => WebhookListenerTransport::Http,
            },
        }
    }

    pub(crate) fn bind_addr(&self) -> std::net::SocketAddr {
        self.bind_addr
    }

    pub(crate) fn transport(&self) -> &WebhookListenerTransport {
        &self.transport
    }
}

pub(crate) enum WebhookListenerTransport {
    Http,
    Https {
        cert_path: PathBuf,
        key_path: PathBuf,
        client_ca_path: Option<PathBuf>,
    },
}

impl WebhookListenerTransport {
    pub(crate) fn effective_mode(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https { client_ca_path, .. } if client_ca_path.is_some() => "https+mtls",
            Self::Https { .. } => "https",
        }
    }

    pub(crate) fn tls(&self) -> Option<WebhookListenerTls<'_>> {
        match self {
            Self::Http => None,
            Self::Https {
                cert_path,
                key_path,
                client_ca_path,
            } => Some(WebhookListenerTls {
                cert_path,
                key_path,
                client_ca_path: client_ca_path.as_ref(),
            }),
        }
    }
}

pub(crate) struct WebhookListenerTls<'a> {
    cert_path: &'a PathBuf,
    key_path: &'a PathBuf,
    client_ca_path: Option<&'a PathBuf>,
}

impl WebhookListenerTls<'_> {
    pub(crate) fn cert_path(&self) -> &std::path::Path {
        self.cert_path
    }

    pub(crate) fn key_path(&self) -> &std::path::Path {
        self.key_path
    }

    pub(crate) fn client_ca_path(&self) -> Option<&std::path::Path> {
        self.client_ca_path.map(|path| path.as_path())
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
