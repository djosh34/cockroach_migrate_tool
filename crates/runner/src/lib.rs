mod config;
mod error;
mod helper_plan;
mod postgres_bootstrap;
mod postgres_setup;
mod schema_compare;
mod sql_name;
mod validated_schema;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use config::LoadedRunnerConfig;
pub use error::RunnerError;
use helper_plan::{HelperPlanArtifacts, render_helper_plan};
use postgres_bootstrap::{PostgresBootstrapReport, bootstrap_postgres};
use postgres_setup::{PostgresSetupArtifacts, render_postgres_setup};
use schema_compare::{SchemaCompareSummary, compare_mapping_exports};

#[derive(Debug, Parser)]
#[command(
    name = "runner",
    about = "CockroachDB to PostgreSQL destination runner"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    ValidateConfig {
        #[arg(long)]
        config: PathBuf,
    },
    RenderPostgresSetup {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
    },
    CompareSchema {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        mapping: String,
        #[arg(long)]
        cockroach_schema: PathBuf,
        #[arg(long)]
        postgres_schema: PathBuf,
    },
    RenderHelperPlan {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        mapping: String,
        #[arg(long)]
        cockroach_schema: PathBuf,
        #[arg(long)]
        postgres_schema: PathBuf,
        #[arg(long)]
        output_dir: PathBuf,
    },
    Run {
        #[arg(long)]
        config: PathBuf,
    },
}

pub fn execute(cli: Cli) -> Result<CommandOutput, RunnerError> {
    match cli.command {
        Command::ValidateConfig { config } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(CommandOutput::Validated(ValidatedConfig::from(&config)))
        }
        Command::RenderPostgresSetup { config, output_dir } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(CommandOutput::PostgresSetupArtifacts(
                render_postgres_setup(&config, &output_dir)?,
            ))
        }
        Command::CompareSchema {
            config,
            mapping,
            cockroach_schema,
            postgres_schema,
        } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(CommandOutput::SchemaCompare(compare_mapping_exports(
                &config,
                &mapping,
                &cockroach_schema,
                &postgres_schema,
            )?))
        }
        Command::RenderHelperPlan {
            config,
            mapping,
            cockroach_schema,
            postgres_schema,
            output_dir,
        } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(CommandOutput::HelperPlanArtifacts(render_helper_plan(
                &config,
                &mapping,
                &cockroach_schema,
                &postgres_schema,
                &output_dir,
            )?))
        }
        Command::Run { config } => {
            let config = LoadedRunnerConfig::load(&config)?;
            let bootstrap = bootstrap_postgres(&config)?;
            Ok(CommandOutput::Startup(RunnerStartupSummary::new(
                &config,
                &bootstrap,
            )))
        }
    }
}

pub enum CommandOutput {
    Validated(ValidatedConfig),
    PostgresSetupArtifacts(PostgresSetupArtifacts),
    SchemaCompare(SchemaCompareSummary),
    HelperPlanArtifacts(HelperPlanArtifacts),
    Startup(RunnerStartupSummary),
}

impl std::fmt::Display for CommandOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validated(config) => config.fmt(f),
            Self::PostgresSetupArtifacts(summary) => summary.fmt(f),
            Self::SchemaCompare(summary) => summary.fmt(f),
            Self::HelperPlanArtifacts(summary) => summary.fmt(f),
            Self::Startup(summary) => summary.fmt(f),
        }
    }
}

pub struct ValidatedConfig {
    config_path: String,
    mappings: usize,
    verify: String,
    webhook_bind_addr: std::net::SocketAddr,
    webhook_tls_files: String,
}

impl From<&LoadedRunnerConfig> for ValidatedConfig {
    fn from(loaded_config: &LoadedRunnerConfig) -> Self {
        let config = loaded_config.config();

        Self {
            config_path: loaded_config.path().display().to_string(),
            mappings: config.mapping_count(),
            verify: config.verify_label(),
            webhook_bind_addr: config.webhook().bind_addr(),
            webhook_tls_files: config.webhook().tls_material_label(),
        }
    }
}

impl std::fmt::Display for ValidatedConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "config valid: config={} mappings={} verify={} webhook={} tls={}",
            self.config_path,
            self.mappings,
            self.verify,
            self.webhook_bind_addr,
            self.webhook_tls_files
        )
    }
}

pub struct RunnerStartupSummary {
    config_path: String,
    mappings: usize,
    mapping_labels: String,
    verify: String,
    webhook_bind_addr: std::net::SocketAddr,
    webhook_tls_files: String,
    reconcile_interval: std::time::Duration,
    bootstrapped_mappings: usize,
}

impl RunnerStartupSummary {
    fn new(
        loaded_config: &LoadedRunnerConfig,
        bootstrap: &PostgresBootstrapReport,
    ) -> Self {
        let config = loaded_config.config();

        Self {
            config_path: loaded_config.path().display().to_string(),
            mappings: config.mapping_count(),
            mapping_labels: config.mapping_labels(),
            verify: config.verify_label(),
            webhook_bind_addr: config.webhook().bind_addr(),
            webhook_tls_files: config.webhook().tls_material_label(),
            reconcile_interval: std::time::Duration::from_secs(config.reconcile().interval_secs()),
            bootstrapped_mappings: bootstrap.bootstrapped_mappings(),
        }
    }
}

impl std::fmt::Display for RunnerStartupSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "runner ready: config={} mappings={} bootstrapped={} labels={} verify={} webhook={} tls={} reconcile={}s",
            self.config_path,
            self.mappings,
            self.bootstrapped_mappings,
            self.mapping_labels,
            self.verify,
            self.webhook_bind_addr,
            self.webhook_tls_files,
            self.reconcile_interval.as_secs()
        )
    }
}
