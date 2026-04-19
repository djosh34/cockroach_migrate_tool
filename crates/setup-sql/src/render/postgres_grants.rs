use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Display, Formatter},
};

use crate::{
    OutputFormat,
    config::PostgresGrantsConfig,
    sql_name::{QualifiedTableName, SqlIdentifier},
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(crate) struct RenderedPostgresGrants {
    databases: Vec<DatabaseGrantPlan>,
}

impl RenderedPostgresGrants {
    pub(crate) fn from_config(config: &PostgresGrantsConfig) -> Self {
        let mut statements_by_database: BTreeMap<String, BTreeSet<PostgresGrantStatement>> =
            BTreeMap::new();

        for mapping in config.mappings() {
            let database = mapping.destination().database().to_owned();
            let runtime_role = SqlIdentifier::new(mapping.destination().runtime_role());
            let destination_database = SqlIdentifier::new(mapping.destination().database());
            let statements = statements_by_database.entry(database).or_default();

            statements.insert(PostgresGrantStatement::DatabaseConnectCreate {
                database: destination_database,
                role: runtime_role.clone(),
            });
            statements.insert(PostgresGrantStatement::SchemaUsage {
                schema: SqlIdentifier::new("public"),
                role: runtime_role.clone(),
            });

            for table in mapping.destination().tables() {
                statements.insert(PostgresGrantStatement::TableMutation {
                    role: runtime_role.clone(),
                    table: QualifiedTableName::new(
                        SqlIdentifier::new(table.schema()),
                        SqlIdentifier::new(table.name()),
                    ),
                });
            }
        }

        Self {
            databases: statements_by_database
                .into_iter()
                .map(|(database, statements)| DatabaseGrantPlan {
                    database,
                    statements: statements.into_iter().collect(),
                })
                .collect(),
        }
    }

    pub(crate) fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Text => self
                .databases
                .iter()
                .map(DatabaseGrantPlan::render_text)
                .collect::<Vec<_>>()
                .join("\n\n"),
            OutputFormat::Json => serde_json::to_string_pretty(
                &self
                    .databases
                    .iter()
                    .map(|database| (database.database.clone(), database.render_sql()))
                    .collect::<BTreeMap<_, _>>(),
            )
            .expect("rendered postgres grants JSON should serialize"),
        }
    }
}

struct DatabaseGrantPlan {
    database: String,
    statements: Vec<PostgresGrantStatement>,
}

impl DatabaseGrantPlan {
    fn render_text(&self) -> String {
        let mut lines = vec![
            "-- PostgreSQL grants SQL".to_owned(),
            format!("-- Destination database: {}", self.database),
            format!("-- Helper schema: {HELPER_SCHEMA}"),
            String::new(),
        ];
        lines.extend(self.statements.iter().map(ToString::to_string));
        lines.join("\n")
    }

    fn render_sql(&self) -> String {
        self.statements
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PostgresGrantStatement {
    DatabaseConnectCreate {
        database: SqlIdentifier,
        role: SqlIdentifier,
    },
    SchemaUsage {
        schema: SqlIdentifier,
        role: SqlIdentifier,
    },
    TableMutation {
        role: SqlIdentifier,
        table: QualifiedTableName,
    },
}

impl Display for PostgresGrantStatement {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DatabaseConnectCreate { database, role } => {
                write!(f, "GRANT CONNECT, CREATE ON DATABASE {database} TO {role};")
            }
            Self::SchemaUsage { schema, role } => {
                write!(f, "GRANT USAGE ON SCHEMA {schema} TO {role};")
            }
            Self::TableMutation { role, table } => write!(
                f,
                "GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE {table} TO {role};"
            ),
        }
    }
}
