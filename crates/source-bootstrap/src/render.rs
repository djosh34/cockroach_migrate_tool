use crate::config::{BootstrapConfig, SourceMapping};

pub(crate) struct RenderedScript {
    text: String,
}

impl RenderedScript {
    pub(crate) fn from_config(config: &BootstrapConfig) -> Self {
        let mut lines = vec![
            "#!/usr/bin/env bash".to_owned(),
            "set -euo pipefail".to_owned(),
            String::new(),
            format!("COCKROACH_URL={}", shell_quote(config.cockroach_url())),
            format!("WEBHOOK_URL={}", shell_quote(config.webhook().url())),
            String::new(),
            "cockroach sql --url \"$COCKROACH_URL\" --execute \"SET CLUSTER SETTING kv.rangefeed.enabled = true;\"".to_owned(),
            "START_CURSOR=$(cockroach sql --url \"$COCKROACH_URL\" --execute \"SELECT cluster_logical_timestamp();\" --format=csv | tail -n +2 | tr -d '\\r')".to_owned(),
            "printf 'starting_cursor=%s\\n' \"$START_CURSOR\"".to_owned(),
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

impl std::fmt::Display for RenderedScript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

fn render_mapping_block(config: &BootstrapConfig, mapping: &SourceMapping) -> Vec<String> {
    let table_list = mapping
        .source()
        .tables()
        .iter()
        .map(|table| table.sql_reference())
        .collect::<Vec<_>>()
        .join(", ");
    let job_var = render_job_variable_name(mapping.id());
    let selected_tables = mapping
        .source()
        .tables()
        .iter()
        .map(|table| table.display_name())
        .collect::<Vec<_>>()
        .join(", ");
    let changefeed_sql = format!(
        "CREATE CHANGEFEED FOR TABLE {table_list} INTO 'webhook-$WEBHOOK_URL' WITH cursor = '$START_CURSOR', initial_scan = 'yes', envelope = 'enriched', enriched_properties = 'source', resolved = {};",
        sql_literal(config.webhook().resolved()),
    );

    vec![
        format!("# Mapping: {}", mapping.id()),
        format!("# Source database: {}", mapping.source().database()),
        format!("# Selected tables: {selected_tables}"),
        format!(
            "{job_var}=$(cockroach sql --url \"$COCKROACH_URL\" --database {} --execute \"{changefeed_sql}\" --format=csv | tail -n +2 | cut -d, -f1 | tr -d '\\r')",
            shell_quote(mapping.source().database()),
        ),
        format!(
            "printf 'mapping_id={} source_database={} selected_tables={} starting_cursor=%s job_id=%s\\n' \"$START_CURSOR\" \"${{{job_var}}}\"",
            mapping.id(),
            mapping.source().database(),
            selected_tables,
        ),
    ]
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r"'\''"))
}

fn sql_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn render_job_variable_name(mapping_id: &str) -> String {
    let suffix: String = mapping_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect();

    format!("JOB_ID_{suffix}")
}
