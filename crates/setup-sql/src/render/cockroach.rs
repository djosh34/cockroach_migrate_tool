use std::collections::BTreeMap;

use crate::{
    OutputFormat,
    config::{BootstrapConfig, SourceMapping},
};

pub(crate) struct RenderedBootstrap {
    databases: Vec<RenderedDatabaseSql>,
}

impl RenderedBootstrap {
    pub(crate) fn from_config(config: &BootstrapConfig) -> Self {
        let shared_prefix = vec![
            "-- Source bootstrap SQL".to_owned(),
            format!("-- Cockroach URL: {}", config.cockroach_url()),
            "-- Apply each statement with a Cockroach SQL client against the source cluster."
                .to_owned(),
            String::new(),
            "SET CLUSTER SETTING kv.rangefeed.enabled = true;".to_owned(),
            "SELECT cluster_logical_timestamp();".to_owned(),
        ];
        let mut grouped_mapping_lines: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for mapping in config.mappings() {
            grouped_mapping_lines
                .entry(mapping.source().database().to_owned())
                .or_default()
                .extend(render_mapping_block(config, mapping));
        }

        Self {
            databases: grouped_mapping_lines
                .into_iter()
                .map(|(database, mut mapping_lines)| {
                    let mut lines = shared_prefix.clone();
                    lines.push(String::new());
                    lines.push(format!("-- Source database: {database}"));
                    lines.append(&mut mapping_lines);
                    RenderedDatabaseSql {
                        database,
                        sql: lines.join("\n"),
                    }
                })
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

fn render_mapping_block(config: &BootstrapConfig, mapping: &SourceMapping) -> Vec<String> {
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
        "CREATE CHANGEFEED FOR TABLE {table_list} INTO {} WITH initial_scan = 'yes', envelope = 'enriched', enriched_properties = 'source', resolved = {};",
        sql_literal(&sink_url),
        sql_literal(config.webhook().resolved()),
    );

    vec![
        format!("-- Mapping: {}", mapping.id()),
        format!("-- Selected tables: {selected_tables}"),
        changefeed_sql,
    ]
}

fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
