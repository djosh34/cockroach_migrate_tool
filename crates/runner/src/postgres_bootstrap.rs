use std::collections::BTreeMap;

use sqlx::{Executor, PgConnection};

use crate::{
    config::PostgresTargetConfig,
    destination_catalog::{close_target, connect_target, load_destination_schema},
    error::RunnerBootstrapError,
    helper_plan::MappingHelperPlan,
    runtime_plan::{ConfiguredMappingPlan, DestinationGroupPlan, RunnerStartupPlan},
    tracking_state::seed_tracking_state,
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
    let mut postgres = connect_target(first_mapping.mapping_id(), destination_group.target())
        .await
        .map_err(RunnerBootstrapError::from)?;
    bootstrap_destination_scaffold(&mut postgres, first_mapping, destination_group.target())
        .await?;

    for mapping in destination_group.mappings() {
        let helper_plan = bootstrap_mapping(&mut postgres, mapping).await?;
        helper_plans.insert(mapping.mapping_id().to_owned(), helper_plan);
    }

    close_target(
        postgres,
        first_mapping.mapping_id(),
        destination_group.target(),
    )
    .await
    .map_err(RunnerBootstrapError::from)?;

    Ok(())
}

async fn bootstrap_destination_scaffold(
    postgres: &mut PgConnection,
    first_mapping: &ConfiguredMappingPlan,
    destination: &PostgresTargetConfig,
) -> Result<(), RunnerBootstrapError> {
    postgres
        .execute(format!("CREATE SCHEMA IF NOT EXISTS {HELPER_SCHEMA}").as_str())
        .await
        .map_err(|source| RunnerBootstrapError::ExecuteDdl {
            mapping_id: first_mapping.mapping_id().to_owned(),
            database: destination.database().to_owned(),
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
            database: destination.database().to_owned(),
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
            database: destination.database().to_owned(),
            source,
        })?;

    Ok(())
}

async fn bootstrap_mapping(
    postgres: &mut PgConnection,
    mapping: &ConfiguredMappingPlan,
) -> Result<MappingHelperPlan, RunnerBootstrapError> {
    let database = mapping.destination().database().to_owned();

    let destination_schema = load_destination_schema(postgres, mapping).await?;
    let helper_plan = MappingHelperPlan::from_validated_schema(
        mapping.mapping_id(),
        mapping.selected_tables(),
        &destination_schema,
    )
    .map_err(|source| RunnerBootstrapError::HelperPlan {
        mapping_id: mapping.mapping_id().to_owned(),
        database: database.clone(),
        source,
    })?;

    for helper_table in helper_plan.helper_tables() {
        postgres
            .execute(helper_table.create_shadow_table_sql().as_str())
            .await
            .map_err(|source| RunnerBootstrapError::ExecuteDdl {
                mapping_id: mapping.mapping_id().to_owned(),
                database: database.clone(),
                source,
            })?;

        if let Some(create_index_sql) = helper_table.create_primary_key_index_sql() {
            postgres
                .execute(create_index_sql.as_str())
                .await
                .map_err(|source| RunnerBootstrapError::ExecuteDdl {
                    mapping_id: mapping.mapping_id().to_owned(),
                    database: database.clone(),
                    source,
                })?;
        }
    }

    seed_tracking_state(
        postgres,
        mapping.mapping_id(),
        mapping.source_database(),
        helper_plan.helper_tables(),
    )
    .await
    .map_err(|source| RunnerBootstrapError::SeedTrackingState {
        mapping_id: mapping.mapping_id().to_owned(),
        database,
        source,
    })?;

    Ok(helper_plan)
}
