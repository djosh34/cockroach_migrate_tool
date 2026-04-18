mod config;
mod error;
mod postgres;
mod reconcile;
mod webhook;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use config::RunnerConfig;
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
            let config = RunnerConfig::load(&config)?;
            Ok(CommandOutput::Validated(ValidatedConfig::from(config)))
        }
        Command::Run { config } => {
            let config = RunnerConfig::load(&config)?;
            let app = RunnerApp::from_config(config);
            Ok(CommandOutput::Startup(app.startup_summary()))
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
    postgres_endpoint: String,
    webhook_bind_addr: std::net::SocketAddr,
}

impl From<RunnerConfig> for ValidatedConfig {
    fn from(config: RunnerConfig) -> Self {
        Self {
            postgres_endpoint: config.postgres().endpoint_label(),
            webhook_bind_addr: config.webhook().bind_addr(),
        }
    }
}

impl std::fmt::Display for ValidatedConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "config valid for postgres {} and webhook {}",
            self.postgres_endpoint, self.webhook_bind_addr
        )
    }
}

struct RunnerApp {
    postgres: PostgresRuntime,
    webhook: WebhookRuntime,
    reconcile: ReconcileRuntime,
}

impl RunnerApp {
    fn from_config(config: RunnerConfig) -> Self {
        Self {
            postgres: PostgresRuntime::from_config(config.postgres()),
            webhook: WebhookRuntime::from_config(config.webhook()),
            reconcile: ReconcileRuntime::from_config(config.reconcile()),
        }
    }

    fn startup_summary(&self) -> RunnerStartupSummary {
        RunnerStartupSummary {
            postgres_endpoint: self.postgres.endpoint_label().to_owned(),
            webhook_bind_addr: self.webhook.bind_addr(),
            webhook_tls_files: self.webhook.tls_material_label(),
            reconcile_interval: self.reconcile.interval(),
        }
    }
}

pub struct RunnerStartupSummary {
    postgres_endpoint: String,
    webhook_bind_addr: std::net::SocketAddr,
    webhook_tls_files: String,
    reconcile_interval: std::time::Duration,
}

impl std::fmt::Display for RunnerStartupSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "runner ready: postgres={} webhook={} tls={} reconcile={}s",
            self.postgres_endpoint,
            self.webhook_bind_addr,
            self.webhook_tls_files,
            self.reconcile_interval.as_secs()
        )
    }
}
