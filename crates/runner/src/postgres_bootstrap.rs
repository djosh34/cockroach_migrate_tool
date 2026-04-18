use std::collections::BTreeMap;

use sqlx::{Connection, Executor, PgConnection, Row};

use crate::{
    config::PostgresConnectionConfig,
    error::RunnerBootstrapError,
    helper_plan::MappingHelperPlan,
    runtime_plan::{ConfiguredMappingPlan, DestinationGroupPlan, RunnerStartupPlan},
    sql_name::{QualifiedTableName, SqlIdentifier},
    tracking_state::seed_tracking_state,
    validated_schema::{
        ColumnSchema, ForeignKeyAction, ForeignKeyShape, PrimaryKeyShape, TableSchema,
        ValidatedSchema,
    },
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(crate) async fn bootstrap_postgres(
    startup_plan: &RunnerStartupPlan,
) -> Result<BTreeMap<String, MappingHelperPlan>, RunnerBootstrapError> {
    bootstrap_all_mappings(startup_plan).await
}

async fn bootstrap_all_mappings(
    startup_plan: &RunnerStartupPlan,
) -> Result<BTreeMap<String, MappingHelperPlan>, RunnerBootstrapError> {
    let mut helper_plans = BTreeMap::new();

    for destination_group in startup_plan.destination_groups() {
        bootstrap_destination_group(destination_group, &mut helper_plans).await?;
    }

    Ok(helper_plans)
}

async fn bootstrap_destination_group(
    destination_group: &DestinationGroupPlan,
    helper_plans: &mut BTreeMap<String, MappingHelperPlan>,
) -> Result<(), RunnerBootstrapError> {
    let first_mapping = destination_group
        .mappings()
        .first()
        .unwrap_or_else(|| panic!("destination group should contain at least one mapping"));
    let mut postgres = PgConnection::connect_with(&destination_group.connection().connect_options())
        .await
        .map_err(|source| RunnerBootstrapError::Connect {
            mapping_id: first_mapping.mapping_id().to_owned(),
            endpoint: destination_group.connection().endpoint_label(),
            source,
        })?;
    bootstrap_destination_scaffold(&mut postgres, first_mapping, destination_group.connection()).await?;

    for mapping in destination_group.mappings().iter().map(MappingBootstrapPlan::from_mapping) {
        let helper_plan = bootstrap_mapping(&mut postgres, &mapping).await?;
        helper_plans.insert(mapping.mapping_id.clone(), helper_plan);
    }

    postgres
        .close()
        .await
        .map_err(|source| RunnerBootstrapError::Connect {
            mapping_id: first_mapping.mapping_id().to_owned(),
            endpoint: destination_group.connection().endpoint_label(),
            source,
        })?;

    Ok(())
}

async fn bootstrap_destination_scaffold(
    postgres: &mut PgConnection,
    first_mapping: &ConfiguredMappingPlan,
    connection: &PostgresConnectionConfig,
) -> Result<(), RunnerBootstrapError> {
    postgres
        .execute(format!("CREATE SCHEMA IF NOT EXISTS {HELPER_SCHEMA}").as_str())
        .await
        .map_err(|source| RunnerBootstrapError::ExecuteDdl {
            mapping_id: first_mapping.mapping_id().to_owned(),
            database: connection.database().to_owned(),
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
            mapping_id: first_mapping.mapping_id().to_owned(),
            database: connection.database().to_owned(),
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
            mapping_id: first_mapping.mapping_id().to_owned(),
            database: connection.database().to_owned(),
            source,
        })?;

    Ok(())
}

async fn bootstrap_mapping(
    postgres: &mut PgConnection,
    mapping: &MappingBootstrapPlan<'_>,
) -> Result<MappingHelperPlan, RunnerBootstrapError> {
    let database = mapping.connection.database().to_owned();

    let destination_schema = load_destination_schema(postgres, mapping).await?;
    let helper_plan = MappingHelperPlan::from_validated_schema(
        &mapping.mapping_id,
        &mapping.selected_tables,
        &destination_schema,
    )
    .map_err(|source| RunnerBootstrapError::HelperPlan {
        mapping_id: mapping.mapping_id.clone(),
        database: database.clone(),
        source,
    })?;

    for helper_table in helper_plan.helper_tables() {
        postgres
            .execute(helper_table.create_shadow_table_sql().as_str())
            .await
            .map_err(|source| RunnerBootstrapError::ExecuteDdl {
                mapping_id: mapping.mapping_id.clone(),
                database: database.clone(),
                source,
            })?;

        if let Some(create_index_sql) = helper_table.create_primary_key_index_sql() {
            postgres
                .execute(create_index_sql.as_str())
                .await
                .map_err(|source| RunnerBootstrapError::ExecuteDdl {
                    mapping_id: mapping.mapping_id.clone(),
                    database: database.clone(),
                    source,
            })?;
        }
    }

    seed_tracking_state(
        postgres,
        &mapping.mapping_id,
        &mapping.source_database,
        helper_plan.helper_tables(),
    )
    .await
    .map_err(|source| RunnerBootstrapError::SeedTrackingState {
        mapping_id: mapping.mapping_id.clone(),
        database,
        source,
    })?;

    Ok(helper_plan)
}

async fn load_destination_schema(
    postgres: &mut PgConnection,
    mapping: &MappingBootstrapPlan<'_>,
) -> Result<ValidatedSchema, RunnerBootstrapError> {
    let mut schema = ValidatedSchema::default();

    for table_name in &mapping.selected_tables {
        let columns = load_table_columns(postgres, mapping, table_name).await?;
        if columns.is_empty() {
            return Err(RunnerBootstrapError::MissingTable {
                mapping_id: mapping.mapping_id.clone(),
                database: mapping.connection.database().to_owned(),
                table: table_name.label(),
            });
        }

        let primary_key_columns = load_primary_key_columns(postgres, mapping, table_name).await?;
        let foreign_keys = load_foreign_keys(postgres, mapping, table_name).await?;

        let mut table_schema = TableSchema::default();
        for column in columns {
            table_schema.push_column(column);
        }
        if !primary_key_columns.is_empty() {
            table_schema.set_primary_key(PrimaryKeyShape::new(primary_key_columns));
        }
        for foreign_key in foreign_keys {
            table_schema.push_foreign_key(foreign_key);
        }

        schema.insert_table(table_name.clone(), table_schema);
    }

    Ok(schema)
}

async fn load_table_columns(
    postgres: &mut PgConnection,
    mapping: &MappingBootstrapPlan<'_>,
    table_name: &QualifiedTableName,
) -> Result<Vec<ColumnSchema>, RunnerBootstrapError> {
    let rows = sqlx::query(
        r#"
        SELECT
            attribute.attname AS column_name,
            pg_catalog.format_type(attribute.atttypid, attribute.atttypmod) AS raw_type,
            NOT attribute.attnotnull AS nullable,
            attribute.attgenerated <> '' AS generated
        FROM pg_attribute AS attribute
        JOIN pg_class AS relation
          ON relation.oid = attribute.attrelid
        JOIN pg_namespace AS namespace
          ON namespace.oid = relation.relnamespace
        WHERE namespace.nspname = $1
          AND relation.relname = $2
          AND attribute.attnum > 0
          AND NOT attribute.attisdropped
        ORDER BY attribute.attnum
        "#,
    )
    .bind(table_name.schema().raw())
    .bind(table_name.table().raw())
    .fetch_all(postgres)
    .await
    .map_err(|source| RunnerBootstrapError::ReadCatalog {
        mapping_id: mapping.mapping_id.clone(),
        database: mapping.connection.database().to_owned(),
        table: table_name.label(),
        source,
    })?;

    Ok(rows
        .into_iter()
        .map(|row| {
            ColumnSchema::new(
                SqlIdentifier::new(&row.get::<String, _>("column_name")),
                row.get::<String, _>("raw_type"),
                row.get::<bool, _>("nullable"),
                row.get::<bool, _>("generated"),
            )
        })
        .collect())
}

async fn load_primary_key_columns(
    postgres: &mut PgConnection,
    mapping: &MappingBootstrapPlan<'_>,
    table_name: &QualifiedTableName,
) -> Result<Vec<SqlIdentifier>, RunnerBootstrapError> {
    let rows = sqlx::query(
        r#"
        SELECT attribute.attname
        FROM pg_constraint AS table_constraint
        JOIN pg_class AS relation
          ON relation.oid = table_constraint.conrelid
        JOIN pg_namespace AS namespace
          ON namespace.oid = relation.relnamespace
        JOIN unnest(table_constraint.conkey) WITH ORDINALITY AS key_columns(attnum, ordinality)
          ON TRUE
        JOIN pg_attribute AS attribute
          ON attribute.attrelid = relation.oid
         AND attribute.attnum = key_columns.attnum
        WHERE table_constraint.contype = 'p'
          AND namespace.nspname = $1
          AND relation.relname = $2
        ORDER BY key_columns.ordinality
        "#,
    )
    .bind(table_name.schema().raw())
    .bind(table_name.table().raw())
    .fetch_all(postgres)
    .await
    .map_err(|source| RunnerBootstrapError::ReadCatalog {
        mapping_id: mapping.mapping_id.clone(),
        database: mapping.connection.database().to_owned(),
        table: table_name.label(),
        source,
    })?;

    Ok(rows
        .into_iter()
        .map(|row| SqlIdentifier::new(&row.get::<String, _>("attname")))
        .collect())
}

async fn load_foreign_keys(
    postgres: &mut PgConnection,
    mapping: &MappingBootstrapPlan<'_>,
    table_name: &QualifiedTableName,
) -> Result<Vec<ForeignKeyShape>, RunnerBootstrapError> {
    let rows = sqlx::query(
        r#"
        SELECT
            table_constraint.conname AS constraint_name,
            source_attribute.attname AS source_column,
            referenced_namespace.nspname AS referenced_schema,
            referenced_relation.relname AS referenced_table,
            referenced_attribute.attname AS referenced_column,
            table_constraint.confdeltype::text AS on_delete
        FROM pg_constraint AS table_constraint
        JOIN pg_class AS relation
          ON relation.oid = table_constraint.conrelid
        JOIN pg_namespace AS namespace
          ON namespace.oid = relation.relnamespace
        JOIN pg_class AS referenced_relation
          ON referenced_relation.oid = table_constraint.confrelid
        JOIN pg_namespace AS referenced_namespace
          ON referenced_namespace.oid = referenced_relation.relnamespace
        JOIN unnest(table_constraint.conkey, table_constraint.confkey) WITH ORDINALITY AS key_columns(source_attnum, referenced_attnum, ordinality)
          ON TRUE
        JOIN pg_attribute AS source_attribute
          ON source_attribute.attrelid = relation.oid
         AND source_attribute.attnum = key_columns.source_attnum
        JOIN pg_attribute AS referenced_attribute
          ON referenced_attribute.attrelid = referenced_relation.oid
         AND referenced_attribute.attnum = key_columns.referenced_attnum
        WHERE table_constraint.contype = 'f'
          AND namespace.nspname = $1
          AND relation.relname = $2
        ORDER BY table_constraint.conname, key_columns.ordinality
        "#,
    )
    .bind(table_name.schema().raw())
    .bind(table_name.table().raw())
    .fetch_all(postgres)
    .await
    .map_err(|source| RunnerBootstrapError::ReadCatalog {
        mapping_id: mapping.mapping_id.clone(),
        database: mapping.connection.database().to_owned(),
        table: table_name.label(),
        source,
    })?;

    let mut foreign_keys = Vec::new();
    let mut current_name = None::<String>;
    let mut current_source_columns = Vec::<SqlIdentifier>::new();
    let mut current_referenced_columns = Vec::<SqlIdentifier>::new();
    let mut current_referenced_table = None::<QualifiedTableName>;
    let mut current_on_delete = None::<ForeignKeyAction>;

    for row in rows {
        let constraint_name = row.get::<String, _>("constraint_name");
        if current_name.as_deref() != Some(constraint_name.as_str()) {
            if current_name.is_some() {
                let Some(referenced_table) = current_referenced_table.take() else {
                    return Err(RunnerBootstrapError::IncompleteForeignKeyMetadata {
                        mapping_id: mapping.mapping_id.clone(),
                        database: mapping.connection.database().to_owned(),
                        table: table_name.label(),
                    });
                };
                let Some(on_delete) = current_on_delete.take() else {
                    return Err(RunnerBootstrapError::IncompleteForeignKeyMetadata {
                        mapping_id: mapping.mapping_id.clone(),
                        database: mapping.connection.database().to_owned(),
                        table: table_name.label(),
                    });
                };
                foreign_keys.push(ForeignKeyShape::new(
                    current_source_columns,
                    referenced_table,
                    current_referenced_columns,
                    on_delete,
                ));
                current_source_columns = Vec::new();
                current_referenced_columns = Vec::new();
            }
            current_name = Some(constraint_name);
            current_referenced_table = Some(QualifiedTableName::new(
                SqlIdentifier::new(&row.get::<String, _>("referenced_schema")),
                SqlIdentifier::new(&row.get::<String, _>("referenced_table")),
            ));
            current_on_delete = Some(parse_on_delete_action(
                &row.get::<String, _>("on_delete"),
                mapping,
                table_name,
            )?);
        }

        current_source_columns.push(SqlIdentifier::new(&row.get::<String, _>("source_column")));
        current_referenced_columns
            .push(SqlIdentifier::new(&row.get::<String, _>("referenced_column")));
    }

    if current_name.is_some() {
        let Some(referenced_table) = current_referenced_table.take() else {
            return Err(RunnerBootstrapError::IncompleteForeignKeyMetadata {
                mapping_id: mapping.mapping_id.clone(),
                database: mapping.connection.database().to_owned(),
                table: table_name.label(),
            });
        };
        let Some(on_delete) = current_on_delete.take() else {
            return Err(RunnerBootstrapError::IncompleteForeignKeyMetadata {
                mapping_id: mapping.mapping_id.clone(),
                database: mapping.connection.database().to_owned(),
                table: table_name.label(),
            });
        };
        foreign_keys.push(ForeignKeyShape::new(
            current_source_columns,
            referenced_table,
            current_referenced_columns,
            on_delete,
        ));
    }

    Ok(foreign_keys)
}

fn parse_on_delete_action(
    value: &str,
    mapping: &MappingBootstrapPlan<'_>,
    table_name: &QualifiedTableName,
) -> Result<ForeignKeyAction, RunnerBootstrapError> {
    match value {
        "a" => Ok(ForeignKeyAction::NoAction),
        "c" => Ok(ForeignKeyAction::Cascade),
        "n" => Ok(ForeignKeyAction::SetNull),
        "r" => Ok(ForeignKeyAction::Restrict),
        other => Err(RunnerBootstrapError::UnsupportedForeignKeyAction {
            mapping_id: mapping.mapping_id.clone(),
            database: mapping.connection.database().to_owned(),
            table: table_name.label(),
            action: other.to_owned(),
        }),
    }
}

struct MappingBootstrapPlan<'a> {
    mapping_id: String,
    source_database: String,
    connection: &'a PostgresConnectionConfig,
    selected_tables: Vec<QualifiedTableName>,
}

impl<'a> MappingBootstrapPlan<'a> {
    fn from_mapping(mapping: &'a ConfiguredMappingPlan) -> Self {
        Self {
            mapping_id: mapping.mapping_id().to_owned(),
            source_database: mapping.source_database().to_owned(),
            connection: mapping.destination_connection(),
            selected_tables: mapping.selected_tables().to_vec(),
        }
    }
}
