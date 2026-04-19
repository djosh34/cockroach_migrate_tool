mod config;
mod error;
mod render;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use config::BootstrapConfig;
pub use error::BootstrapError;
use render::RenderedBootstrap;

#[derive(Debug, Parser)]
#[command(name = "source-bootstrap", about = "CockroachDB source bootstrap CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    RenderBootstrapSql {
        #[arg(long)]
        config: PathBuf,
    },
}

pub fn execute(cli: Cli) -> Result<CommandOutput, BootstrapError> {
    match cli.command {
        Command::RenderBootstrapSql { config } => {
            let config = BootstrapConfig::load(&config)?;
            Ok(RenderedBootstrap::from_config(&config).to_string())
        }
    }
}

pub type CommandOutput = String;
