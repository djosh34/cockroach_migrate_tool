use std::collections::BTreeMap;

use crate::{
    OutputFormat,
    config::{BootstrapConfig, SourceMapping},
};

const CHANGEFEED_CURSOR_PLACEHOLDER: &str = "__CHANGEFEED_CURSOR__";
const CHANGEFEED_CURSOR_CAPTURE_SQL: &str =
    "SELECT cluster_logical_timestamp() AS changefeed_cursor;";

pub(crate) struct RenderedBootstrap {
    databases: Vec<RenderedDatabaseSql>,
}

impl RenderedBootstrap {
    pub(crate) fn from_config(config: &BootstrapConfig) -> Self {
        let plan = CockroachSetupPlan::from_config(config);
        Self {
            databases: plan
                .databases
                .into_iter()
                .map(RenderedDatabaseSql::from_plan)
                .collect(),
        }
    }

    pub(crate) fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Text => self
                .databases
                .iter()
                .map(|database| database.sql.as_str())
                .collect::<Vec<_>>()
                .join("\n\n"),
            OutputFormat::Json => serde_json::to_string_pretty(
                &self
                    .databases
                    .iter()
                    .map(|database| (database.database.clone(), database.sql.clone()))
                    .collect::<BTreeMap<_, _>>(),
            )
            .expect("rendered bootstrap JSON should serialize"),
        }
    }
}

struct RenderedDatabaseSql {
    database: String,
    sql: String,
}

impl RenderedDatabaseSql {
    fn from_plan(plan: DatabaseSetupPlan) -> Self {
        let sql = plan.render_sql();
        Self {
            database: plan.database,
            sql,
        }
    }
}

struct CockroachSetupPlan {
    databases: Vec<DatabaseSetupPlan>,
}

impl CockroachSetupPlan {
    fn from_config(config: &BootstrapConfig) -> Self {
        let mut mappings_by_database: BTreeMap<String, Vec<ChangefeedSetupPlan>> = BTreeMap::new();

        for mapping in config.mappings() {
            mappings_by_database
                .entry(mapping.source().database().to_owned())
                .or_default()
                .push(ChangefeedSetupPlan::from_mapping(config, mapping));
        }

        Self {
            databases: mappings_by_database
                .into_iter()
                .map(|(database, changefeeds)| DatabaseSetupPlan {
                    database,
                    cockroach_url: config.cockroach_url().to_owned(),
                    changefeeds,
                })
                .collect(),
        }
    }
}

struct DatabaseSetupPlan {
    database: String,
    cockroach_url: String,
    changefeeds: Vec<ChangefeedSetupPlan>,
}

impl DatabaseSetupPlan {
    fn render_sql(&self) -> String {
        let mut lines = vec![
            "-- Source bootstrap SQL".to_owned(),
            format!("-- Cockroach URL: {}", self.cockroach_url),
            "-- Apply each statement with a Cockroach SQL client against the source cluster."
                .to_owned(),
            format!(
                "-- Capture the cursor once, then replace {CHANGEFEED_CURSOR_PLACEHOLDER} in the CREATE CHANGEFEED statements below."
            ),
            String::new(),
            "SET CLUSTER SETTING kv.rangefeed.enabled = true;".to_owned(),
            CHANGEFEED_CURSOR_CAPTURE_SQL.to_owned(),
            String::new(),
            format!("-- Source database: {}", self.database),
        ];

        for changefeed in &self.changefeeds {
            lines.push(String::new());
            lines.extend(changefeed.render_lines());
        }

        lines.join("\n")
    }
}

struct ChangefeedSetupPlan {
    mapping_id: String,
    selected_tables: String,
    changefeed_sql: String,
}

impl ChangefeedSetupPlan {
    fn from_mapping(config: &BootstrapConfig, mapping: &SourceMapping) -> Self {
        let selected_tables = mapping
            .source()
            .tables()
            .iter()
            .map(|table| table.display_name())
            .collect::<Vec<_>>()
            .join(", ");
        let table_list = mapping
            .source()
            .tables()
            .iter()
            .map(|table| table.sql_reference_in_database(mapping.source().database()))
            .collect::<Vec<_>>()
            .join(", ");
        let sink_url = format!(
            "webhook-{}{}",
            config.webhook().base_url(),
            config.webhook().changefeed_sink_suffix(mapping.id())
        );
        let changefeed_sql = format!(
            "CREATE CHANGEFEED FOR TABLE {table_list} INTO {} WITH cursor = {}, initial_scan = 'yes', envelope = 'enriched', enriched_properties = 'source', resolved = {};",
            sql_literal(&sink_url),
            sql_literal(CHANGEFEED_CURSOR_PLACEHOLDER),
            sql_literal(config.webhook().resolved()),
        );

        Self {
            mapping_id: mapping.id().to_owned(),
            selected_tables,
            changefeed_sql,
        }
    }

    fn render_lines(&self) -> Vec<String> {
        vec![
            format!("-- Mapping: {}", self.mapping_id),
            format!("-- Selected tables: {}", self.selected_tables),
            format!(
                "-- Replace {CHANGEFEED_CURSOR_PLACEHOLDER} below with the decimal cursor returned above before running the CREATE CHANGEFEED statement."
            ),
            self.changefeed_sql.clone(),
        ]
    }
}

fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
