mod error;
mod helper_plan;
mod metrics;
mod postgres_bootstrap;
mod reconcile_runtime;
mod runtime_plan;
mod tracking_state;
mod webhook_runtime;

use std::{ffi::OsString, path::PathBuf, sync::Arc};

use operator_log::{LogEvent, LogFormat};
use runner_config::{
    LoadedRunnerConfig, RunnerStartupPlan, ValidatedConfig, validate_loaded_config,
};

pub use error::RunnerError;
use postgres_bootstrap::bootstrap_postgres;
use reconcile_runtime::serve as serve_reconcile_runtime;
use runtime_plan::RunnerRuntimePlan;
use webhook_runtime::serve as serve_webhook_runtime;

pub(crate) type RuntimeEventSink = Arc<dyn Fn(LogEvent<'static>) + Send + Sync>;

#[derive(Debug)]
pub struct Cli {
    log_format: LogFormat,
    command: Command,
}

#[derive(Debug)]
enum Command {
    ValidateConfig { config: PathBuf, deep: bool },
    Run { config: PathBuf },
}

pub async fn execute<F>(cli: Cli, emit_event: F) -> Result<Option<CommandOutput>, RunnerError>
where
    F: Fn(LogEvent<'static>) + Send + Sync + 'static,
{
    let emit_event: RuntimeEventSink = Arc::new(emit_event);

    match cli.command {
        Command::ValidateConfig { config, deep } => {
            let config = LoadedRunnerConfig::load(&config)?;
            Ok(Some(CommandOutput::Validated(
                validate_loaded_config(&config, deep).await?,
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
    pub fn parse_from_env() -> Result<Self, CliError> {
        Self::parse_from(std::env::args_os())
    }

    pub fn log_format(&self) -> LogFormat {
        self.log_format
    }

    fn parse_from<I>(args: I) -> Result<Self, CliError>
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut args = args.into_iter();
        let program = args
            .next()
            .unwrap_or_else(|| OsString::from("runner"))
            .to_string_lossy()
            .into_owned();
        let usage = usage_text(&program);
        let mut command_name: Option<String> = None;
        let mut config: Option<PathBuf> = None;
        let mut deep = false;
        let mut log_format = LogFormat::Text;

        while let Some(argument) = args.next() {
            let argument = argument.to_string_lossy().into_owned();
            match argument.as_str() {
                "-h" | "--help" => return Err(CliError::help(usage)),
                "--log-format" => {
                    let value = next_string_argument(&mut args, "--log-format", &usage)?;
                    log_format = parse_log_format(&value)
                        .map_err(|message| CliError::invalid(message, &usage))?;
                }
                "--config" => {
                    let value = args
                        .next()
                        .ok_or_else(|| CliError::missing_value("--config", &usage))?;
                    config = Some(PathBuf::from(value));
                }
                "--deep" => {
                    deep = true;
                }
                "validate-config" | "run" => {
                    if let Some(existing) = &command_name {
                        return Err(CliError::invalid(
                            format!("received multiple subcommands: `{existing}` and `{argument}`"),
                            &usage,
                        ));
                    }
                    command_name = Some(argument);
                }
                _ if argument.starts_with('-') => {
                    return Err(CliError::invalid(
                        format!("unrecognized argument `{argument}`"),
                        &usage,
                    ));
                }
                _ => {
                    return Err(CliError::invalid(
                        format!("unrecognized subcommand `{argument}`"),
                        &usage,
                    ));
                }
            }
        }

        let command_name =
            command_name.ok_or_else(|| CliError::invalid("missing subcommand", &usage))?;
        let config = config.ok_or_else(|| CliError::missing_value("--config", &usage))?;
        let command = match command_name.as_str() {
            "validate-config" => Command::ValidateConfig { config, deep },
            "run" => {
                if deep {
                    return Err(CliError::invalid(
                        "`--deep` is only valid for `validate-config`",
                        &usage,
                    ));
                }
                Command::Run { config }
            }
            _ => unreachable!("command name validated during parsing"),
        };

        Ok(Self {
            log_format,
            command,
        })
    }
}

fn parse_log_format(input: &str) -> Result<LogFormat, String> {
    match input {
        "text" => Ok(LogFormat::Text),
        "json" => Ok(LogFormat::Json),
        _ => Err(format!(
            "invalid log format `{input}`; expected one of: text, json"
        )),
    }
}

fn next_string_argument(
    args: &mut impl Iterator<Item = OsString>,
    flag: &str,
    usage: &str,
) -> Result<String, CliError> {
    let value = args
        .next()
        .ok_or_else(|| CliError::missing_value(flag, usage))?;
    Ok(value.to_string_lossy().into_owned())
}

fn usage_text(program: &str) -> String {
    format!(
        "CockroachDB to PostgreSQL destination runner\n\n\
Usage:\n  {program} [--log-format text|json] validate-config --config <PATH> [--deep]\n  {program} [--log-format text|json] run --config <PATH>\n\n\
Commands:\n  validate-config   Validate a runner config file\n  run               Start the webhook and reconcile runtimes\n\n\
Options:\n  --log-format <text|json>  Emit text or JSON logs (default: text)\n  --config <PATH>           Path to the runner config file\n  --deep                    Validate destination connectivity and schema\n  -h, --help                Show this help text"
    )
}

#[derive(Debug)]
pub struct CliError {
    kind: CliErrorKind,
}

#[derive(Debug)]
enum CliErrorKind {
    Help(String),
    InvalidUsage(String),
}

impl CliError {
    fn help(usage: String) -> Self {
        Self {
            kind: CliErrorKind::Help(usage),
        }
    }

    fn invalid(message: impl Into<String>, usage: &str) -> Self {
        Self {
            kind: CliErrorKind::InvalidUsage(format!("error: {}\n\n{usage}", message.into())),
        }
    }

    fn missing_value(flag: &str, usage: &str) -> Self {
        Self::invalid(format!("missing value for `{flag}`"), usage)
    }

    pub fn is_help(&self) -> bool {
        matches!(self.kind, CliErrorKind::Help(_))
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.kind {
            CliErrorKind::Help(usage) | CliErrorKind::InvalidUsage(usage) => f.write_str(usage),
        }
    }
}

impl std::error::Error for CliError {}

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
