mod config;
mod error;
mod helper_plan;
mod molt_verify;
mod postgres_bootstrap;
mod postgres_setup;
mod reconcile_runtime;
mod runtime_plan;
mod schema_compare;
mod sql_name;
mod tracking_state;
mod validated_schema;
mod webhook_runtime;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use config::LoadedRunnerConfig;
pub use error::RunnerError;
use helper_plan::{HelperPlanArtifacts, render_helper_plan};
use molt_verify::{MoltVerifySummary, run_verify};
use postgres_bootstrap::bootstrap_postgres;
use postgres_setup::{PostgresSetupArtifacts, render_postgres_setup};
use reconcile_runtime::serve as serve_reconcile_runtime;
use runtime_plan::{RunnerRuntimePlan, RunnerStartupPlan};
use schema_compare::{SchemaCompareSummary, compare_mapping_exports};
use webhook_runtime::serve as serve_webhook_runtime;

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
    Verify {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        mapping: String,
        #[arg(long)]
        source_url: String,
        #[arg(long, default_value_t = false)]
        allow_tls_mode_disable: bool,
    },
    Run {
        #[arg(long)]
        config: PathBuf,
    },
}

pub async fn execute(cli: Cli) -> Result<Option<CommandOutput>, RunnerError> {
    match cli.command {
        Command::ValidateConfig { config } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(Some(CommandOutput::Validated(ValidatedConfig::from(&config))))
        }
        Command::RenderPostgresSetup { config, output_dir } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(Some(CommandOutput::PostgresSetupArtifacts(
                render_postgres_setup(&config, &output_dir)?,
            )))
        }
        Command::CompareSchema {
            config,
            mapping,
            cockroach_schema,
            postgres_schema,
        } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(Some(CommandOutput::SchemaCompare(compare_mapping_exports(
                &config,
                &mapping,
                &cockroach_schema,
                &postgres_schema,
            )?)))
        }
        Command::RenderHelperPlan {
            config,
            mapping,
            cockroach_schema,
            postgres_schema,
            output_dir,
        } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(Some(CommandOutput::HelperPlanArtifacts(render_helper_plan(
                &config,
                &mapping,
                &cockroach_schema,
                &postgres_schema,
                &output_dir,
            )?)))
        }
        Command::Verify {
            config,
            mapping,
            source_url,
            allow_tls_mode_disable,
        } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(Some(CommandOutput::MoltVerify(run_verify(
                &config,
                &mapping,
                &source_url,
                allow_tls_mode_disable,
            )?)))
        }
        Command::Run { config } => {
            let config = LoadedRunnerConfig::load(&config)?;
            let startup_plan = RunnerStartupPlan::from_config(config.config())?;
            let helper_plans = bootstrap_postgres(&startup_plan).await?;
            let runtime = RunnerRuntimePlan::from_startup_plan(startup_plan, helper_plans)?;
            let runtime = std::sync::Arc::new(runtime);
            tokio::try_join!(
                async {
                    serve_webhook_runtime(runtime.clone())
                        .await
                        .map_err(RunnerError::from)
                },
                async {
                    serve_reconcile_runtime(runtime.clone())
                        .await
                        .map_err(RunnerError::from)
                }
            )?;
            Ok(None)
        }
    }
}

pub enum CommandOutput {
    Validated(ValidatedConfig),
    PostgresSetupArtifacts(PostgresSetupArtifacts),
    SchemaCompare(SchemaCompareSummary),
    HelperPlanArtifacts(HelperPlanArtifacts),
    MoltVerify(MoltVerifySummary),
}

impl std::fmt::Display for CommandOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validated(config) => config.fmt(f),
            Self::PostgresSetupArtifacts(summary) => summary.fmt(f),
            Self::SchemaCompare(summary) => summary.fmt(f),
            Self::HelperPlanArtifacts(summary) => summary.fmt(f),
            Self::MoltVerify(summary) => summary.fmt(f),
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
