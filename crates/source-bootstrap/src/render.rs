use crate::config::{BootstrapConfig, SourceMapping};

pub(crate) struct RenderedBootstrap {
    text: String,
}

impl RenderedBootstrap {
    pub(crate) fn from_config(config: &BootstrapConfig) -> Self {
        let mut lines = vec![
            "-- Source bootstrap SQL".to_owned(),
            format!("-- Cockroach URL: {}", config.cockroach_url()),
            "-- Apply each statement with a Cockroach SQL client against the source cluster."
                .to_owned(),
            String::new(),
            "SET CLUSTER SETTING kv.rangefeed.enabled = true;".to_owned(),
            "SELECT cluster_logical_timestamp();".to_owned(),
        ];

        for mapping in config.mappings() {
            lines.push(String::new());
            lines.extend(render_mapping_block(config, mapping));
        }

        Self {
            text: lines.join("\n"),
        }
    }
}

impl std::fmt::Display for RenderedBootstrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
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
        format!("-- Source database: {}", mapping.source().database()),
        format!("-- Selected tables: {selected_tables}"),
        changefeed_sql,
    ]
}

fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
