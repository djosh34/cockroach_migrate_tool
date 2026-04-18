use std::{collections::{BTreeMap, BTreeSet}, path::PathBuf};

use crate::{
    config::RunnerConfig,
    error::RunnerWebhookRoutingError,
    webhook_runtime::payload::{ResolvedRequest, RowBatchRequest, RowEvent, WebhookRequest},
};

pub(crate) struct RunnerWebhookPlan {
    bind_addr: std::net::SocketAddr,
    tls_cert_path: PathBuf,
    tls_key_path: PathBuf,
    _reconcile_interval_secs: u64,
    routes: BTreeMap<String, MappingWebhookRoute>,
}

impl RunnerWebhookPlan {
    pub(crate) fn from_config(config: &RunnerConfig) -> Self {
        Self {
            bind_addr: config.webhook().bind_addr(),
            tls_cert_path: config.webhook().tls().cert_path().to_path_buf(),
            tls_key_path: config.webhook().tls().key_path().to_path_buf(),
            _reconcile_interval_secs: config.reconcile().interval_secs(),
            routes: config
                .mappings()
                .iter()
                .map(|mapping| {
                    (
                        mapping.id().to_owned(),
                        MappingWebhookRoute::new(
                            mapping.id(),
                            mapping.source().database(),
                            mapping.source().tables(),
                        ),
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
    allowed_tables: BTreeSet<String>,
}

impl MappingWebhookRoute {
    fn new(mapping_id: &str, source_database: &str, allowed_tables: &[String]) -> Self {
        Self {
            mapping_id: mapping_id.to_owned(),
            source_database: source_database.to_owned(),
            allowed_tables: allowed_tables.iter().cloned().collect(),
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
        let mut selected_table = None::<String>;
        for row in batch.rows() {
            let Some(source) = row.source() else {
                return Err(RunnerWebhookRoutingError::MissingSource {
                    mapping_id: self.mapping_id.clone(),
                });
            };
            if source.database_name() != self.source_database {
                return Err(RunnerWebhookRoutingError::SourceDatabaseMismatch {
                    mapping_id: self.mapping_id.clone(),
                    expected: self.source_database.clone(),
                    database: source.database_name().to_owned(),
                });
            }

            let table = source.table_label();
            if !self.allowed_tables.contains(&table) {
                return Err(RunnerWebhookRoutingError::SourceTableNotMapped {
                    mapping_id: self.mapping_id.clone(),
                    table,
                });
            }

            match &selected_table {
                Some(existing) if existing != &table => {
                    return Err(RunnerWebhookRoutingError::MixedSourceTables {
                        mapping_id: self.mapping_id.clone(),
                        first: existing.clone(),
                        second: table,
                    });
                }
                Some(_) => {}
                None => selected_table = Some(table),
            }
        }

        Ok(DispatchTarget::RowBatch {
            mapping_id: self.mapping_id.clone(),
            table: selected_table.unwrap_or_default(),
            rows: batch.rows().to_vec(),
        })
    }
}

pub(crate) enum DispatchTarget {
    RowBatch {
        mapping_id: String,
        table: String,
        rows: Vec<RowEvent>,
    },
    Resolved {
        mapping_id: String,
        resolved: String,
    }
}
