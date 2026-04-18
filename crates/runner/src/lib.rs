mod config;
mod error;
mod postgres;
mod reconcile;
mod webhook;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use config::{LoadedRunnerConfig, RunnerConfig};
pub use error::RunnerError;
use postgres::PostgresRuntime;
use reconcile::ReconcileRuntime;
use webhook::WebhookRuntime;

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
        Command::Run { config } => {
            let config = LoadedRunnerConfig::load(&config)?;
            let app = RunnerApp::from_config(config.config());
            Ok(CommandOutput::Startup(app.startup_summary(config.path())))
        }
    }
}

pub enum CommandOutput {
    Validated(ValidatedConfig),
    Startup(RunnerStartupSummary),
}

impl std::fmt::Display for CommandOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Validated(config) => config.fmt(f),
            Self::Startup(summary) => summary.fmt(f),
        }
    }
}

pub struct ValidatedConfig {
    config_path: String,
    postgres_endpoint: String,
    webhook_bind_addr: std::net::SocketAddr,
    webhook_tls_files: String,
}

impl From<&LoadedRunnerConfig> for ValidatedConfig {
    fn from(loaded_config: &LoadedRunnerConfig) -> Self {
        let config = loaded_config.config();
        Self {
            config_path: loaded_config.path().display().to_string(),
            postgres_endpoint: config.postgres().endpoint_label(),
            webhook_bind_addr: config.webhook().bind_addr(),
            webhook_tls_files: config.webhook().tls_material_label(),
        }
    }
}

impl std::fmt::Display for ValidatedConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "config valid: config={} postgres={} webhook={} tls={}",
            self.config_path,
            self.postgres_endpoint,
            self.webhook_bind_addr,
            self.webhook_tls_files
        )
    }
}

struct RunnerApp {
    postgres: PostgresRuntime,
    webhook: WebhookRuntime,
    reconcile: ReconcileRuntime,
}

impl RunnerApp {
    fn from_config(config: &RunnerConfig) -> Self {
        Self {
            postgres: PostgresRuntime::from_config(config.postgres()),
            webhook: WebhookRuntime::from_config(config.webhook()),
            reconcile: ReconcileRuntime::from_config(config.reconcile()),
        }
    }

    fn startup_summary(&self, config_path: &std::path::Path) -> RunnerStartupSummary {
        RunnerStartupSummary {
            config_path: config_path.display().to_string(),
            postgres_endpoint: self.postgres.endpoint_label().to_owned(),
            webhook_bind_addr: self.webhook.bind_addr(),
            webhook_tls_files: self.webhook.tls_material_label().to_owned(),
            reconcile_interval: self.reconcile.interval(),
        }
    }
}

pub struct RunnerStartupSummary {
    config_path: String,
    postgres_endpoint: String,
    webhook_bind_addr: std::net::SocketAddr,
    webhook_tls_files: String,
    reconcile_interval: std::time::Duration,
}

impl std::fmt::Display for RunnerStartupSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "runner ready: config={} postgres={} webhook={} tls={} reconcile={}s",
            self.config_path,
            self.postgres_endpoint,
            self.webhook_bind_addr,
            self.webhook_tls_files,
            self.reconcile_interval.as_secs()
        )
    }
}
