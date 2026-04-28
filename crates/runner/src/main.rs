use operator_log::{LogEvent, LogFormat};
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = match runner::Cli::parse_from_env() {
        Ok(cli) => cli,
        Err(error) => {
            if error.is_help() {
                println!("{error}");
                return ExitCode::SUCCESS;
            }
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let log_format = cli.log_format();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            write_stderr_event(
                LogEvent::error(
                    "runner",
                    "runtime.start_failed",
                    format!("failed to start async runtime: {error}"),
                ),
                log_format,
            );
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(runner::execute(cli, move |event| {
        write_stderr_event(event, log_format);
    })) {
        Ok(Some(output)) => {
            if log_format.writes_json() {
                write_stderr_event(output.event(), log_format);
            } else {
                println!("{}", output.text_output());
            }
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(error) => {
            write_stderr_event(
                LogEvent::error("runner", "command.failed", error.to_string()),
                log_format,

            );
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
