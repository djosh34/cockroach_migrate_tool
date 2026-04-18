use std::{collections::BTreeMap, path::PathBuf};

use crate::{
    config::{MappingConfig, PostgresConnectionConfig, RunnerConfig},
    error::RunnerWebhookRoutingError,
    helper_plan::{HelperShadowTablePlan, MappingHelperPlan},
    webhook_runtime::{
        payload::{ResolvedRequest, RowBatchRequest, WebhookRequest},
        persistence::RowMutationBatch,
    },
};

pub(crate) struct RunnerWebhookPlan {
    bind_addr: std::net::SocketAddr,
    tls_cert_path: PathBuf,
    tls_key_path: PathBuf,
    _reconcile_interval_secs: u64,
    routes: BTreeMap<String, MappingWebhookRoute>,
}

impl RunnerWebhookPlan {
    pub(crate) fn from_config(
        config: &RunnerConfig,
        mut helper_plans: BTreeMap<String, MappingHelperPlan>,
    ) -> Self {
        Self {
            bind_addr: config.webhook().bind_addr(),
            tls_cert_path: config.webhook().tls().cert_path().to_path_buf(),
            tls_key_path: config.webhook().tls().key_path().to_path_buf(),
            _reconcile_interval_secs: config.reconcile().interval_secs(),
            routes: config
                .mappings()
                .iter()
                .map(|mapping| {
                    let helper_plan = helper_plans
                        .remove(mapping.id())
                        .expect("bootstrap should produce a helper plan for every mapping");
                    (
                        mapping.id().to_owned(),
                        MappingWebhookRoute::new(mapping, helper_plan),
                    )
                })
                .collect(),
        }
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

    pub(crate) fn route(&self, mapping_id: &str) -> Option<&MappingWebhookRoute> {
        self.routes.get(mapping_id)
    }

    pub(crate) fn require_route(
        &self,
        mapping_id: &str,
    ) -> Result<&MappingWebhookRoute, RunnerWebhookRoutingError> {
        self.route(mapping_id)
            .ok_or_else(|| RunnerWebhookRoutingError::UnknownMapping {
                mapping_id: mapping_id.to_owned(),
            })
    }
}

pub(crate) struct MappingWebhookRoute {
    mapping_id: String,
    source_database: String,
    destination_connection: PostgresConnectionConfig,
    tables: BTreeMap<String, HelperShadowTablePlan>,
}

impl MappingWebhookRoute {
    fn new(mapping: &MappingConfig, helper_plan: MappingHelperPlan) -> Self {
        Self {
            mapping_id: mapping.id().to_owned(),
            source_database: mapping.source().database().to_owned(),
            destination_connection: mapping.destination().connection().clone(),
            tables: helper_plan
                .helper_tables()
                .iter()
                .cloned()
                .map(|table| (table.source_table().label(), table))
                .collect(),
        }
    }

    fn route_resolved(&self, resolved: ResolvedRequest) -> DispatchTarget {
        DispatchTarget::Resolved {
            mapping_id: self.mapping_id.clone(),
            resolved: resolved.resolved().to_owned(),
        }
    }

    pub(crate) fn route_request(
        &self,
        request: WebhookRequest,
    ) -> Result<DispatchTarget, RunnerWebhookRoutingError> {
        match request {
            WebhookRequest::Resolved(resolved) => Ok(self.route_resolved(resolved)),
            WebhookRequest::RowBatch(batch) => self.route_row_batch(batch),
        }
    }

    fn route_row_batch(
        &self,
        batch: RowBatchRequest,
    ) -> Result<DispatchTarget, RunnerWebhookRoutingError> {
        let mut selected_table = None::<HelperShadowTablePlan>;
        for row in batch.rows() {
            let source = row.source();
            if source.database_name() != self.source_database {
                return Err(RunnerWebhookRoutingError::SourceDatabaseMismatch {
                    mapping_id: self.mapping_id.clone(),
                    expected: self.source_database.clone(),
                    database: source.database_name().to_owned(),
                });
            }

            let table = source.table_label();
            let helper_table =
                self.tables
                    .get(&table)
                    .ok_or_else(|| RunnerWebhookRoutingError::SourceTableNotMapped {
                        mapping_id: self.mapping_id.clone(),
                        table: table.clone(),
                    })?;

            match &selected_table {
                Some(existing) if existing.source_table().label() != table => {
                    return Err(RunnerWebhookRoutingError::MixedSourceTables {
                        mapping_id: self.mapping_id.clone(),
                        first: existing.source_table().label(),
                        second: table,
                    });
                }
                Some(_) => {}
                None => selected_table = Some(helper_table.clone()),
            }
        }

        let selected_table = selected_table.ok_or_else(|| RunnerWebhookRoutingError::EmptyRowBatch {
            mapping_id: self.mapping_id.clone(),
        })?;

        Ok(DispatchTarget::RowBatch(Box::new(RowMutationBatch {
            mapping_id: self.mapping_id.clone(),
            connection: self.destination_connection.clone(),
            table: selected_table,
            rows: batch
                .into_rows()
                .into_iter()
                .map(|row| row.into_mutation())
                .collect(),
        })))
    }
}

pub(crate) enum DispatchTarget {
    RowBatch(Box<RowMutationBatch>),
    Resolved {
        mapping_id: String,
        resolved: String,
    },
}
