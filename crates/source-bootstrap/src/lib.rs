mod config;
mod error;
mod render;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use config::BootstrapConfig;
pub use error::BootstrapError;
use render::RenderedScript;

#[derive(Debug, Parser)]
#[command(name = "source-bootstrap", about = "CockroachDB source bootstrap CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    RenderBootstrapScript {
        #[arg(long)]
        config: PathBuf,
    },
}

pub fn execute(cli: Cli) -> Result<CommandOutput, BootstrapError> {
    match cli.command {
        Command::RenderBootstrapScript { config } => {
            let config = BootstrapConfig::load(&config)?;
            Ok(RenderedScript::from_config(&config).to_string())
        }
    }
}

pub type CommandOutput = String;
