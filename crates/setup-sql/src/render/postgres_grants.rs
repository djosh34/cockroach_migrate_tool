use std::collections::BTreeMap;

use crate::{
    OutputFormat,
    config::{PostgresGrantMapping, PostgresGrantsConfig},
    sql_name::{QualifiedTableName, SqlIdentifier},
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(crate) struct RenderedPostgresGrants {
    databases: Vec<RenderedDatabaseSql>,
}

impl RenderedPostgresGrants {
    pub(crate) fn from_config(config: &PostgresGrantsConfig) -> Self {
        let mut mappings_by_database: BTreeMap<String, Vec<&PostgresGrantMapping>> = BTreeMap::new();

        for mapping in config.mappings() {
            mappings_by_database
                .entry(mapping.destination().database().to_owned())
                .or_default()
                .push(mapping);
        }

        Self {
            databases: mappings_by_database
                .into_iter()
                .map(|(database, mappings)| RenderedDatabaseSql {
                    sql: render_database_sql(&database, &mappings),
                    database,
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
            .expect("rendered postgres grants JSON should serialize"),
        }
    }
}

struct RenderedDatabaseSql {
    database: String,
    sql: String,
}

fn render_database_sql(database: &str, mappings: &[&PostgresGrantMapping]) -> String {
    let mut lines = vec![
        "-- PostgreSQL grants SQL".to_owned(),
        format!("-- Destination database: {database}"),
        format!("-- Helper schema: {HELPER_SCHEMA}"),
    ];

    for mapping in mappings {
        let runtime_role = SqlIdentifier::new(mapping.destination().runtime_role());
        let destination_database = SqlIdentifier::new(mapping.destination().database());
        lines.push(String::new());
        lines.push(format!("-- Mapping: {}", mapping.id()));
        lines.push(format!("-- Runtime role: {}", mapping.destination().runtime_role()));
        lines.push(format!(
            "GRANT CONNECT, TEMPORARY, CREATE ON DATABASE {} TO {};",
            destination_database, runtime_role
        ));
        lines.push(format!(
            "GRANT USAGE ON SCHEMA public TO {};",
            runtime_role
        ));
        for table in mapping.destination().tables() {
            let table = QualifiedTableName::new(
                SqlIdentifier::new(table.schema()),
                SqlIdentifier::new(table.name()),
            );
            lines.push(format!(
                "GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE {} TO {};",
                table, runtime_role
            ));
        }
    }

    lines.join("\n")
}
