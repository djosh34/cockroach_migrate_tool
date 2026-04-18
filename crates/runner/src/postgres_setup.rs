use std::{
    fmt::{self, Display, Formatter},
    fs,
    path::{Path, PathBuf},
};

use crate::{
    config::{LoadedRunnerConfig, MappingConfig, RunnerConfig},
    error::RunnerArtifactError,
    sql_name::{QualifiedTableName, SqlIdentifier},
};

const HELPER_SCHEMA: &str = "_cockroach_migration_tool";

pub(crate) fn render_postgres_setup(
    loaded_config: &LoadedRunnerConfig,
    output_dir: &Path,
) -> Result<PostgresSetupArtifacts, RunnerArtifactError> {
    let plan = PostgresSetupPlan::from_config(loaded_config.config());
    plan.write_to(output_dir)?;

    Ok(PostgresSetupArtifacts {
        output_dir: output_dir.to_path_buf(),
        mappings: plan.mapping_count(),
    })
}

pub struct PostgresSetupArtifacts {
    output_dir: PathBuf,
    mappings: usize,
}

impl Display for PostgresSetupArtifacts {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "postgres setup artifacts written: output={} mappings={}",
            self.output_dir.display(),
            self.mappings
        )
    }
}

struct PostgresSetupPlan {
    mappings: Vec<PostgresGrantPlan>,
}

impl PostgresSetupPlan {
    fn from_config(config: &RunnerConfig) -> Self {
        let mappings = config
            .mappings()
            .iter()
            .map(PostgresGrantPlan::from_mapping)
            .collect();

        Self { mappings }
    }

    fn mapping_count(&self) -> usize {
        self.mappings.len()
    }

    fn write_to(&self, output_dir: &Path) -> Result<(), RunnerArtifactError> {
        fs::create_dir_all(output_dir).map_err(|source| {
            RunnerArtifactError::CreateOutputDirectory {
                path: output_dir.to_path_buf(),
                source,
            }
        })?;

        write_artifact(output_dir.join("README.md"), TopLevelReadme(self))?;

        for mapping in &self.mappings {
            let mapping_dir = output_dir.join(mapping.mapping_id.as_str());
            fs::create_dir_all(&mapping_dir).map_err(|source| {
                RunnerArtifactError::CreateMappingDirectory {
                    path: mapping_dir.clone(),
                    source,
                }
            })?;
            write_artifact(mapping_dir.join("grants.sql"), GrantSql(mapping))?;
            write_artifact(mapping_dir.join("README.md"), MappingReadme(mapping))?;
        }

        Ok(())
    }
}

struct PostgresGrantPlan {
    mapping_id: String,
    destination_database: SqlIdentifier,
    runtime_role: SqlIdentifier,
    tables: Vec<QualifiedTableName>,
}

impl PostgresGrantPlan {
    fn from_mapping(mapping: &MappingConfig) -> Self {
        let destination = mapping.destination().connection();
        let tables = mapping
            .source()
            .tables()
            .iter()
            .map(|table| QualifiedTableName::from_config(table))
            .collect();

        Self {
            mapping_id: mapping.id().to_owned(),
            destination_database: SqlIdentifier::new(destination.database()),
            runtime_role: SqlIdentifier::new(destination.user()),
            tables,
        }
    }
}

struct TopLevelReadme<'a>(&'a PostgresSetupPlan);

impl Display for TopLevelReadme<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "# PostgreSQL Setup Artifacts")?;
        writeln!(f)?;
        writeln!(
            f,
            "These artifacts keep PostgreSQL grants explicit and manual. Render them, review them, run each `grants.sql`, then start the runner."
        )?;
        writeln!(f)?;
        writeln!(
            f,
            "Once the grants exist, `runner run --config <path>` bootstraps helper objects inside schema `{HELPER_SCHEMA}` automatically, but it does not create roles or execute grants for you."
        )?;
        writeln!(f)?;
        writeln!(f, "No superuser requirement is assumed or recommended.")?;
        writeln!(f)?;
        writeln!(f, "## Mappings")?;
        writeln!(f)?;
        for mapping in &self.0.mappings {
            writeln!(
                f,
                "- `{}`: database `{}` role `{}`",
                mapping.mapping_id,
                mapping.destination_database.raw(),
                mapping.runtime_role.raw()
            )?;
            writeln!(
                f,
                "  Artifacts: `{0}/grants.sql`, `{0}/README.md`",
                mapping.mapping_id
            )?;
        }

        Ok(())
    }
}

struct MappingReadme<'a>(&'a PostgresGrantPlan);

impl Display for MappingReadme<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mapping = self.0;

        writeln!(f, "# PostgreSQL Setup For `{}`", mapping.mapping_id)?;
        writeln!(f)?;
        writeln!(
            f,
            "Run `grants.sql` while connected to database `{}` before starting the runner.",
            mapping.destination_database.raw()
        )?;
        writeln!(f)?;
        writeln!(
            f,
            "These grants stay manual and explicit by design. The runtime role is `{}`.",
            mapping.runtime_role.raw()
        )?;
        writeln!(f)?;
        writeln!(f, "No superuser requirement is assumed or recommended.")?;
        writeln!(f)?;
        writeln!(
            f,
            "After the grants exist, `runner run --config <path>` creates helper objects in schema `{HELPER_SCHEMA}` automatically."
        )?;
        writeln!(
            f,
            "If `{HELPER_SCHEMA}` already exists, it must already be owned by `{}`.",
            mapping.runtime_role.raw()
        )?;

        Ok(())
    }
}

struct GrantSql<'a>(&'a PostgresGrantPlan);

impl Display for GrantSql<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mapping = self.0;

        writeln!(
            f,
            "-- PostgreSQL grants for mapping `{}`",
            mapping.mapping_id
        )?;
        writeln!(
            f,
            "-- Destination database: {}",
            mapping.destination_database.raw()
        )?;
        writeln!(f, "-- Runtime role: {}", mapping.runtime_role.raw())?;
        writeln!(f, "-- Helper schema: {HELPER_SCHEMA}")?;
        writeln!(f)?;
        writeln!(
            f,
            "GRANT CONNECT, TEMPORARY, CREATE ON DATABASE {} TO {};",
            mapping.destination_database, mapping.runtime_role
        )?;
        writeln!(
            f,
            "GRANT USAGE ON SCHEMA public TO {};",
            mapping.runtime_role
        )?;
        for table in &mapping.tables {
            writeln!(
                f,
                "GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE {} TO {};",
                table, mapping.runtime_role
            )?;
        }

        Ok(())
    }
}

fn write_artifact(path: PathBuf, content: impl Display) -> Result<(), RunnerArtifactError> {
    fs::write(&path, content.to_string())
        .map_err(|source| RunnerArtifactError::WriteFile { path, source })
}
