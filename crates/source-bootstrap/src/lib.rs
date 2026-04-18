mod config;
mod error;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use config::BootstrapConfig;
pub use error::BootstrapError;

#[derive(Debug, Parser)]
#[command(name = "source-bootstrap", about = "CockroachDB source bootstrap CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    CreateChangefeed {
        #[arg(long)]
        config: PathBuf,
    },
}

pub fn execute(cli: Cli) -> Result<CommandOutput, BootstrapError> {
    match cli.command {
        Command::CreateChangefeed { config } => {
            let config = BootstrapConfig::load(&config)?;
            Ok(CommandOutput::CreateChangefeed(BootstrapPlan::from(config)))
        }
    }
}

pub enum CommandOutput {
    CreateChangefeed(BootstrapPlan),
}

impl std::fmt::Display for CommandOutput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateChangefeed(plan) => plan.fmt(f),
        }
    }
}

pub struct BootstrapPlan {
    cursor: String,
    source_url: String,
    tables: String,
    webhook_url: String,
}

impl From<BootstrapConfig> for BootstrapPlan {
    fn from(config: BootstrapConfig) -> Self {
        Self {
            cursor: config.cursor().to_owned(),
            source_url: config.source_url().to_owned(),
            tables: config.tables().join(", "),
            webhook_url: config.webhook_url().to_owned(),
        }
    }
}

impl std::fmt::Display for BootstrapPlan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "bootstrap plan ready: source={} cursor={} tables={} webhook={}",
            self.source_url, self.cursor, self.tables, self.webhook_url
        )
    }
}
