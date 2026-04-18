use sqlx::{Connection, Executor, PgConnection, Row, postgres::PgConnectOptions};

use crate::{
    config::{LoadedRunnerConfig, PostgresConnectionConfig, RunnerConfig},
    error::RunnerBootstrapError,
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(crate) struct PostgresBootstrapReport {
    bootstrapped_mappings: usize,
}

impl PostgresBootstrapReport {
    pub(crate) fn bootstrapped_mappings(&self) -> usize {
        self.bootstrapped_mappings
    }
}

pub(crate) fn bootstrap_postgres(
    loaded_config: &LoadedRunnerConfig,
) -> Result<PostgresBootstrapReport, RunnerBootstrapError> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|source| RunnerBootstrapError::StartRuntime { source })?;

    runtime.block_on(async move {
        bootstrap_all_mappings(loaded_config.config()).await?;
        Ok(PostgresBootstrapReport {
            bootstrapped_mappings: loaded_config.config().mapping_count(),
        })
    })
}

async fn bootstrap_all_mappings(config: &RunnerConfig) -> Result<(), RunnerBootstrapError> {
    for mapping in config
        .mappings()
        .iter()
        .map(MappingBootstrapPlan::from_mapping)
    {
        bootstrap_mapping(&mapping).await?;
    }

    Ok(())
}

async fn bootstrap_mapping(
    mapping: &MappingBootstrapPlan<'_>,
) -> Result<(), RunnerBootstrapError> {
    let endpoint = mapping.connection.endpoint_label();
    let mut postgres = PgConnection::connect_with(
        &PgConnectOptions::new()
            .host(mapping.connection.host())
            .port(mapping.connection.port())
            .database(mapping.connection.database())
            .username(mapping.connection.user())
            .password(mapping.connection.password()),
    )
    .await
    .map_err(|source| RunnerBootstrapError::Connect {
        mapping_id: mapping.mapping_id.clone(),
        endpoint,
        source,
    })?;

    postgres
        .execute(format!("CREATE SCHEMA IF NOT EXISTS {HELPER_SCHEMA}").as_str())
        .await
        .map_err(|source| RunnerBootstrapError::ExecuteDdl {
            mapping_id: mapping.mapping_id.clone(),
            database: mapping.connection.database().to_owned(),
            source,
        })?;

    postgres
        .execute(
            format!(
                "CREATE TABLE IF NOT EXISTS {HELPER_SCHEMA}.stream_state (
                    mapping_id TEXT PRIMARY KEY,
                    source_database TEXT NOT NULL,
                    source_job_id TEXT,
                    starting_cursor TEXT,
                    latest_received_resolved_watermark TEXT,
                    latest_reconciled_resolved_watermark TEXT,
                    stream_status TEXT NOT NULL DEFAULT 'bootstrap_pending'
                )"
            )
            .as_str(),
        )
        .await
        .map_err(|source| RunnerBootstrapError::ExecuteDdl {
            mapping_id: mapping.mapping_id.clone(),
            database: mapping.connection.database().to_owned(),
            source,
        })?;

    postgres
        .execute(
            format!(
                "CREATE TABLE IF NOT EXISTS {HELPER_SCHEMA}.table_sync_state (
                    mapping_id TEXT NOT NULL,
                    source_table_name TEXT NOT NULL,
                    helper_table_name TEXT NOT NULL,
                    last_successful_sync_time TIMESTAMPTZ,
                    last_successful_sync_watermark TEXT,
                    last_error TEXT,
                    PRIMARY KEY (mapping_id, source_table_name)
                )"
            )
            .as_str(),
        )
        .await
        .map_err(|source| RunnerBootstrapError::ExecuteDdl {
            mapping_id: mapping.mapping_id.clone(),
            database: mapping.connection.database().to_owned(),
            source,
        })?;

    for helper_table in &mapping.helper_tables {
        ensure_source_table_exists(&mut postgres, mapping, helper_table).await?;

        let create_shadow_table_sql = helper_table.create_shadow_table_sql();
        postgres
            .execute(create_shadow_table_sql.as_str())
            .await
            .map_err(|source| RunnerBootstrapError::ExecuteDdl {
                mapping_id: mapping.mapping_id.clone(),
                database: mapping.connection.database().to_owned(),
                source,
            })?;

        let primary_key_columns = load_primary_key_columns(&mut postgres, mapping, helper_table).await?;

        if !primary_key_columns.is_empty() {
            let create_helper_index_sql =
                helper_table.create_primary_key_index_sql(&primary_key_columns);
            postgres
                .execute(create_helper_index_sql.as_str())
                .await
                .map_err(|source| RunnerBootstrapError::ExecuteDdl {
                    mapping_id: mapping.mapping_id.clone(),
                    database: mapping.connection.database().to_owned(),
                    source,
                })?;
        }
    }

    postgres.close().await.map_err(|source| RunnerBootstrapError::Connect {
        mapping_id: mapping.mapping_id.clone(),
        endpoint: mapping.connection.endpoint_label(),
        source,
    })?;

    Ok(())
}

async fn ensure_source_table_exists(
    postgres: &mut PgConnection,
    mapping: &MappingBootstrapPlan<'_>,
    helper_table: &HelperTablePlan,
) -> Result<(), RunnerBootstrapError> {
    let row = sqlx::query(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = $1
              AND table_name = $2
        ) AS table_exists
        "#,
    )
    .bind(helper_table.source_table.schema.raw())
    .bind(helper_table.source_table.table.raw())
    .fetch_one(postgres)
    .await
    .map_err(|source| RunnerBootstrapError::ReadCatalog {
        mapping_id: mapping.mapping_id.clone(),
        database: mapping.connection.database().to_owned(),
        table: helper_table.source_table.label(),
        source,
    })?;

    if row.get::<bool, _>("table_exists") {
        Ok(())
    } else {
        Err(RunnerBootstrapError::MissingTable {
            mapping_id: mapping.mapping_id.clone(),
            database: mapping.connection.database().to_owned(),
            table: helper_table.source_table.label(),
        })
    }
}

async fn load_primary_key_columns(
    postgres: &mut PgConnection,
    mapping: &MappingBootstrapPlan<'_>,
    helper_table: &HelperTablePlan,
) -> Result<Vec<SqlIdentifier>, RunnerBootstrapError> {
    let rows = sqlx::query(
        r#"
        SELECT attribute.attname
        FROM pg_constraint AS table_constraint
        JOIN pg_class relation
          ON relation.oid = table_constraint.conrelid
        JOIN pg_namespace namespace
          ON namespace.oid = relation.relnamespace
        JOIN unnest(table_constraint.conkey) WITH ORDINALITY AS key_columns(attnum, ordinality)
          ON TRUE
        JOIN pg_attribute attribute
          ON attribute.attrelid = relation.oid
         AND attribute.attnum = key_columns.attnum
        WHERE table_constraint.contype = 'p'
          AND namespace.nspname = $1
          AND relation.relname = $2
        ORDER BY key_columns.ordinality
        "#,
    )
    .bind(helper_table.source_table.schema.raw())
    .bind(helper_table.source_table.table.raw())
    .fetch_all(postgres)
    .await
    .map_err(|source| RunnerBootstrapError::ReadCatalog {
        mapping_id: mapping.mapping_id.clone(),
        database: mapping.connection.database().to_owned(),
        table: helper_table.source_table.label(),
        source,
    })?;

    Ok(rows
        .into_iter()
        .map(|row| SqlIdentifier::new(&row.get::<String, _>("attname")))
        .collect())
}

struct MappingBootstrapPlan<'a> {
    mapping_id: String,
    connection: &'a PostgresConnectionConfig,
    helper_tables: Vec<HelperTablePlan>,
}

impl<'a> MappingBootstrapPlan<'a> {
    fn from_mapping(mapping: &'a crate::config::MappingConfig) -> Self {
        Self {
            mapping_id: mapping.id().to_owned(),
            connection: mapping.destination().connection(),
            helper_tables: mapping
                .source()
                .tables()
                .iter()
                .map(|table| HelperTablePlan::new(mapping.id(), QualifiedTableName::from_config(table)))
                .collect(),
        }
    }
}

struct HelperTablePlan {
    source_table: QualifiedTableName,
    helper_table_name: String,
}

impl HelperTablePlan {
    fn new(mapping_id: &str, source_table: QualifiedTableName) -> Self {
        let helper_table_name = format!(
            "{mapping_id}__{}__{}",
            source_table.schema.raw(),
            source_table.table.raw()
        );

        Self {
            source_table,
            helper_table_name,
        }
    }

    fn create_shadow_table_sql(&self) -> String {
        format!(
            "CREATE TABLE IF NOT EXISTS {}.{} (LIKE {}.{} INCLUDING DEFAULTS INCLUDING GENERATED)",
            SqlIdentifier::new(HELPER_SCHEMA),
            SqlIdentifier::new(&self.helper_table_name),
            self.source_table.schema,
            self.source_table.table,
        )
    }

    fn create_primary_key_index_sql(&self, primary_key_columns: &[SqlIdentifier]) -> String {
        let columns = primary_key_columns
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "CREATE INDEX IF NOT EXISTS {} ON {}.{} ({columns})",
            SqlIdentifier::new(&format!("{}__pk", self.helper_table_name)),
            SqlIdentifier::new(HELPER_SCHEMA),
            SqlIdentifier::new(&self.helper_table_name),
        )
    }
}

struct QualifiedTableName {
    schema: SqlIdentifier,
    table: SqlIdentifier,
}

impl QualifiedTableName {
    fn from_config(value: &str) -> Self {
        let (schema, table) = value
            .split_once('.')
            .expect("validated config should only contain schema-qualified tables");

        Self {
            schema: SqlIdentifier::new(schema),
            table: SqlIdentifier::new(table),
        }
    }

    fn label(&self) -> String {
        format!("{}.{}", self.schema.raw(), self.table.raw())
    }
}

#[derive(Clone)]
struct SqlIdentifier {
    raw: String,
}

impl SqlIdentifier {
    fn new(value: &str) -> Self {
        Self {
            raw: value.to_owned(),
        }
    }

    fn raw(&self) -> &str {
        &self.raw
    }
}

impl std::fmt::Display for SqlIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\"", self.raw.replace('"', "\"\""))
    }
}
