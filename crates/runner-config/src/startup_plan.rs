use std::{collections::BTreeMap, path::PathBuf, time::Duration};

use crate::{
    config::{MappingConfig, PostgresTargetConfig, RunnerConfig, WebhookConfig},
    error::RunnerStartupPlanError,
    sql_name::QualifiedTableName,
};

pub struct RunnerStartupPlan {
    webhook_listener: WebhookListenerPlan,
    reconcile_interval: Duration,
    mappings: BTreeMap<String, ConfiguredMappingPlan>,
    destination_groups: Vec<DestinationGroupPlan>,
}

impl RunnerStartupPlan {
    pub fn from_config(config: &RunnerConfig) -> Result<Self, RunnerStartupPlanError> {
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
            .collect::<Result<Vec<_>, RunnerStartupPlanError>>()?;

        Ok(Self {
            webhook_listener: WebhookListenerPlan::from_config(config.webhook()),
            reconcile_interval: Duration::from_secs(config.reconcile().interval_secs()),
            mappings,
            destination_groups,
        })
    }

    pub fn destination_groups(&self) -> &[DestinationGroupPlan] {
        &self.destination_groups
    }

    pub fn webhook_listener(&self) -> &WebhookListenerPlan {
        &self.webhook_listener
    }

    pub fn reconcile_interval(&self) -> Duration {
        self.reconcile_interval
    }

    pub fn mappings(&self) -> &BTreeMap<String, ConfiguredMappingPlan> {
        &self.mappings
    }
}

#[derive(Clone)]
pub struct ConfiguredMappingPlan {
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

    pub fn mapping_id(&self) -> &str {
        &self.mapping_id
    }

    pub fn source_database(&self) -> &str {
        &self.source_database
    }

    pub fn destination(&self) -> &PostgresTargetConfig {
        &self.destination
    }

    pub fn selected_tables(&self) -> &[QualifiedTableName] {
        &self.selected_tables
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DestinationDatabaseKey {
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

    fn label(&self) -> String {
        format!("{}:{}/{}", self.host, self.port, self.database)
    }
}

#[derive(Clone)]
pub struct DestinationGroupPlan {
    target: PostgresTargetConfig,
    mappings: Vec<ConfiguredMappingPlan>,
}

impl DestinationGroupPlan {
    fn new(
        database_key: DestinationDatabaseKey,
        mappings: Vec<ConfiguredMappingPlan>,
    ) -> Result<Self, RunnerStartupPlanError> {
        let Some(target) = mappings
            .first()
            .map(|mapping| mapping.destination().clone())
        else {
            panic!("destination group should contain at least one mapping");
        };

        for mapping in &mappings {
            if !mapping.destination().same_target_contract(&target) {
                return Err(RunnerStartupPlanError::InconsistentDestinationTarget {
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
                    return Err(RunnerStartupPlanError::OverlappingDestinationTable {
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

    pub fn target(&self) -> &PostgresTargetConfig {
        &self.target
    }

    pub fn mappings(&self) -> &[ConfiguredMappingPlan] {
        &self.mappings
    }
}

#[derive(Clone)]
pub struct WebhookListenerPlan {
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

    pub fn bind_addr(&self) -> std::net::SocketAddr {
        self.bind_addr
    }

    pub fn transport(&self) -> &WebhookListenerTransport {
        &self.transport
    }
}

#[derive(Clone)]
pub enum WebhookListenerTransport {
    Http,
    Https {
        cert_path: PathBuf,
        key_path: PathBuf,
        client_ca_path: Option<PathBuf>,
    },
}

impl WebhookListenerTransport {
    pub fn effective_mode(&self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Https { client_ca_path, .. } if client_ca_path.is_some() => "https+mtls",
            Self::Https { .. } => "https",
        }
    }

    pub fn tls(&self) -> Option<WebhookListenerTls<'_>> {
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

pub struct WebhookListenerTls<'a> {
    cert_path: &'a PathBuf,
    key_path: &'a PathBuf,
    client_ca_path: Option<&'a PathBuf>,
}

impl WebhookListenerTls<'_> {
    pub fn cert_path(&self) -> &std::path::Path {
        self.cert_path
    }

    pub fn key_path(&self) -> &std::path::Path {
        self.key_path
    }

    pub fn client_ca_path(&self) -> Option<&std::path::Path> {
        self.client_ca_path.map(|path| path.as_path())
    }
}
