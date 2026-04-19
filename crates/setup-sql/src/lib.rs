mod config;
mod error;
mod render;
mod sql_name;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use config::{BootstrapConfig, PostgresGrantsConfig};
pub use error::BootstrapError;
use render::{RenderedBootstrap, RenderedPostgresGrants};

#[derive(Debug, Parser)]
#[command(name = "setup-sql", about = "One-time SQL emission CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    EmitCockroachSql {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    EmitPostgresGrants {
        #[arg(long)]
        config: PathBuf,
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
}

pub fn execute(cli: Cli) -> Result<CommandOutput, BootstrapError> {
    match cli.command {
        Command::EmitCockroachSql { config, format } => {
            let config = BootstrapConfig::load(&config)?;
            Ok(RenderedBootstrap::from_config(&config).render(format))
        }
        Command::EmitPostgresGrants { config, format } => {
            let config = PostgresGrantsConfig::load(&config)?;
            Ok(RenderedPostgresGrants::from_config(&config).render(format))
        }
    }
}

pub type CommandOutput = String;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}
