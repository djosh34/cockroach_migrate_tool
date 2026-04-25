mod config;
mod destination_catalog;
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

use std::{path::PathBuf, sync::Arc};

use clap::{Parser, Subcommand};
use destination_catalog::validate_destination_group;
use operator_log::{LogEvent, LogFormat};

use config::LoadedRunnerConfig;
pub use error::RunnerError;
use postgres_bootstrap::bootstrap_postgres;
use reconcile_runtime::serve as serve_reconcile_runtime;
use runtime_plan::{RunnerRuntimePlan, RunnerStartupPlan};
use webhook_runtime::serve as serve_webhook_runtime;

pub(crate) type RuntimeEventSink = Arc<dyn Fn(LogEvent<'static>) + Send + Sync>;

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
        #[arg(long)]
        deep: bool,
    },
    Run {
        #[arg(long)]
        config: PathBuf,
    },
}

pub async fn execute<F>(cli: Cli, emit_event: F) -> Result<Option<CommandOutput>, RunnerError>
where
    F: Fn(LogEvent<'static>) + Send + Sync + 'static,
{
    let emit_event: RuntimeEventSink = Arc::new(emit_event);

    match cli.command {
        Command::ValidateConfig { config, deep } => {
            let config = LoadedRunnerConfig::load(&config)?;
            let deep_validation = if deep {
                let startup_plan = RunnerStartupPlan::from_config(config.config())?;
                for destination_group in startup_plan.destination_groups() {
                    validate_destination_group(destination_group).await?;
                }
                DeepValidationStatus::Ok
            } else {
                DeepValidationStatus::Skipped
            };
            Ok(Some(CommandOutput::Validated(
                ValidatedConfig::from_loaded_config(&config, deep_validation),
            )))
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
                    serve_webhook_runtime(runtime.clone(), emit_event.clone())
                        .await
                        .map_err(RunnerError::from)
                },
                async {
                    serve_reconcile_runtime(runtime.clone(), emit_event.clone())
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
    webhook_mode: &'static str,
    webhook_tls_files: Option<String>,
    deep_validation: DeepValidationStatus,
}

#[derive(Clone, Copy)]
enum DeepValidationStatus {
    Skipped,
    Ok,
}

impl ValidatedConfig {
    fn from_loaded_config(
        loaded_config: &LoadedRunnerConfig,
        deep_validation: DeepValidationStatus,
    ) -> Self {
        let config = loaded_config.config();

        Self {
            config_path: loaded_config.path().display().to_string(),
            mappings: config.mapping_count(),
            webhook_bind_addr: config.webhook().bind_addr(),
            webhook_mode: config.webhook().effective_mode(),
            webhook_tls_files: config.webhook().tls().map(|tls| tls.material_label()),
            deep_validation,
        }
    }

    fn text_output(&self) -> String {
        let mut summary = format!(
            "config valid: config={} mappings={} webhook={} mode={}",
            self.config_path, self.mappings, self.webhook_bind_addr, self.webhook_mode
        );
        if let Some(tls) = &self.webhook_tls_files {
            summary.push_str(" tls=");
            summary.push_str(tls);
        }
        if let Some(deep_status) = self.deep_validation.field_value() {
            summary.push_str(" deep=");
            summary.push_str(deep_status);
        }
        summary
    }

    fn event(&self) -> LogEvent<'static> {
        let mut event = LogEvent::info("runner", "config.validated", "runner config validated")
            .with_field("config", &self.config_path)
            .with_field("mappings", self.mappings)
            .with_field("webhook", self.webhook_bind_addr.to_string())
            .with_field("mode", self.webhook_mode);
        if let Some(tls) = &self.webhook_tls_files {
            event = event.with_field("tls", tls);
        }
        if let Some(deep_status) = self.deep_validation.field_value() {
            event = event.with_field("deep", deep_status);
        }
        event
    }
}

impl DeepValidationStatus {
    fn field_value(self) -> Option<&'static str> {
        match self {
            Self::Skipped => None,
            Self::Ok => Some("ok"),
        }
    }
}
