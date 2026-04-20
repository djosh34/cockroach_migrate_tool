mod config;
mod error;
mod render;
mod sql_name;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use operator_log::{LogEvent, LogFormat};

use config::{BootstrapConfig, PostgresGrantsConfig};
pub use error::BootstrapError;
use render::{RenderedBootstrap, RenderedPostgresGrants};

#[derive(Debug, Parser)]
#[command(name = "setup-sql", about = "One-time SQL emission CLI")]
pub struct Cli {
    #[arg(long, value_enum, global = true, default_value_t = LogFormat::Text)]
    log_format: LogFormat,
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
            let config_path = config.display().to_string();
            let config = BootstrapConfig::load(&config)?;
            Ok(CommandOutput::new(
                RenderedBootstrap::from_config(&config).render(format),
                CommandEvent::sql_emitted("emit-cockroach-sql", config_path, format),
            ))
        }
        Command::EmitPostgresGrants { config, format } => {
            let config_path = config.display().to_string();
            let config = PostgresGrantsConfig::load(&config)?;
            Ok(CommandOutput::new(
                RenderedPostgresGrants::from_config(&config).render(format),
                CommandEvent::sql_emitted("emit-postgres-grants", config_path, format),
            ))
        }
    }
}

impl Cli {
    pub fn log_format(&self) -> LogFormat {
        self.log_format
    }
}

pub struct CommandOutput {
    payload: String,
    event: CommandEvent,
}

impl CommandOutput {
    fn new(payload: String, event: CommandEvent) -> Self {
        Self { payload, event }
    }

    pub fn payload(&self) -> &str {
        &self.payload
    }

    pub fn event(&self) -> LogEvent<'static> {
        self.event.to_log_event()
    }
}

struct CommandEvent {
    event: &'static str,
    message: &'static str,
    command: &'static str,
    config_path: String,
    payload_format: &'static str,
}

impl CommandEvent {
    fn sql_emitted(
        command: &'static str,
        config_path: String,
        payload_format: OutputFormat,
    ) -> Self {
        Self {
            event: "sql.emitted",
            message: "setup sql emitted",
            command,
            config_path,
            payload_format: payload_format.as_str(),
        }
    }

    fn to_log_event(&self) -> LogEvent<'static> {
        LogEvent::info("setup-sql", self.event, self.message)
            .with_field("command", self.command)
            .with_field("config", &self.config_path)
            .with_field("payload_format", self.payload_format)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

impl OutputFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
        }
    }
}
