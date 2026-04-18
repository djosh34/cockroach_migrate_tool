mod config;
mod error;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use config::LoadedRunnerConfig;
pub use error::RunnerError;

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
            Ok(CommandOutput::Startup(RunnerStartupSummary::from(&config)))
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
}

impl From<&LoadedRunnerConfig> for RunnerStartupSummary {
    fn from(loaded_config: &LoadedRunnerConfig) -> Self {
        let config = loaded_config.config();

        Self {
            config_path: loaded_config.path().display().to_string(),
            mappings: config.mapping_count(),
            mapping_labels: config.mapping_labels(),
            verify: config.verify_label(),
            webhook_bind_addr: config.webhook().bind_addr(),
            webhook_tls_files: config.webhook().tls_material_label(),
            reconcile_interval: std::time::Duration::from_secs(config.reconcile().interval_secs()),
        }
    }
}

impl std::fmt::Display for RunnerStartupSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "runner ready: config={} mappings={} labels={} verify={} webhook={} tls={} reconcile={}s",
            self.config_path,
            self.mappings,
            self.mapping_labels,
            self.verify,
            self.webhook_bind_addr,
            self.webhook_tls_files,
            self.reconcile_interval.as_secs()
        )
    }
}
