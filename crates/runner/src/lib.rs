mod config;
mod error;
mod helper_plan;
mod metrics;
mod postgres_bootstrap;
mod reconcile_runtime;
mod runtime_plan;
mod sql_name;
mod tracking_state;
mod validated_schema;
mod webhook_runtime;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use operator_log::{LogEvent, LogFormat};

use config::LoadedRunnerConfig;
pub use error::RunnerError;
use postgres_bootstrap::bootstrap_postgres;
use reconcile_runtime::serve as serve_reconcile_runtime;
use runtime_plan::{RunnerRuntimePlan, RunnerStartupPlan};
use webhook_runtime::serve as serve_webhook_runtime;

#[derive(Debug, Parser)]
#[command(
    name = "runner",
    about = "CockroachDB to PostgreSQL destination runner"
)]
pub struct Cli {
    #[arg(long, value_enum, global = true, default_value_t = LogFormat::Text)]
    log_format: LogFormat,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    ValidateConfig {
        #[arg(long)]
        config: PathBuf,
    },
    Run {
        #[arg(long)]
        config: PathBuf,
    },
}

pub async fn execute<F>(cli: Cli, mut emit_event: F) -> Result<Option<CommandOutput>, RunnerError>
where
    F: FnMut(LogEvent<'static>),
{
    match cli.command {
        Command::ValidateConfig { config } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(Some(CommandOutput::Validated(ValidatedConfig::from(
                &config,
            ))))
        }
        Command::Run { config } => {
            let config = LoadedRunnerConfig::load(&config)?;
            let startup_plan = RunnerStartupPlan::from_config(config.config())?;
            emit_event(LogEvent::info(
                "runner",
                "runtime.starting",
                "runner runtime starting",
            ));
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

impl Cli {
    pub fn log_format(&self) -> LogFormat {
        self.log_format
    }
}

pub enum CommandOutput {
    Validated(ValidatedConfig),
}

impl CommandOutput {
    pub fn event(&self) -> LogEvent<'static> {
        match self {
            Self::Validated(config) => config.event(),
        }
    }

    pub fn text_output(&self) -> String {
        match self {
            Self::Validated(config) => config.text_output(),
        }
    }
}

pub struct ValidatedConfig {
    config_path: String,
    mappings: usize,
    webhook_bind_addr: std::net::SocketAddr,
    webhook_tls_files: String,
}

impl From<&LoadedRunnerConfig> for ValidatedConfig {
    fn from(loaded_config: &LoadedRunnerConfig) -> Self {
        let config = loaded_config.config();

        Self {
            config_path: loaded_config.path().display().to_string(),
            mappings: config.mapping_count(),
            webhook_bind_addr: config.webhook().bind_addr(),
            webhook_tls_files: config.webhook().tls_material_label(),
        }
    }
}

impl ValidatedConfig {
    fn text_output(&self) -> String {
        format!(
            "config valid: config={} mappings={} webhook={} tls={}",
            self.config_path, self.mappings, self.webhook_bind_addr, self.webhook_tls_files
        )
    }

    fn event(&self) -> LogEvent<'static> {
        LogEvent::info("runner", "config.validated", "runner config validated")
            .with_field("config", &self.config_path)
            .with_field("mappings", self.mappings)
            .with_field("webhook", self.webhook_bind_addr.to_string())
            .with_field("tls", &self.webhook_tls_files)
    }
}
