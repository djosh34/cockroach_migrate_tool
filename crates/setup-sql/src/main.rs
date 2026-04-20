use clap::Parser;
use operator_log::{LogEvent, LogFormat};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = source_bootstrap::Cli::parse();
    let log_format = cli.log_format();
    match source_bootstrap::execute(cli) {
        Ok(output) => {
            println!("{}", output.payload());
            if log_format.writes_json() {
                write_stderr_event(output.event(), log_format);
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if log_format.writes_json() {
                write_stderr_event(
                    LogEvent::error("setup-sql", "command.failed", error.to_string()),
                    log_format,
                );
            } else {
                eprintln!("{error}");
            }
            ExitCode::FAILURE
        }
    }
}

fn write_stderr_event(event: LogEvent<'static>, log_format: LogFormat) {
    let mut stderr = std::io::stderr().lock();
    event
        .write_to(&mut stderr, log_format)
        .expect("stderr log event should be writable");
}
