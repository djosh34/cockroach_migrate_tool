use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = runner::Cli::parse();
    match runner::execute(cli) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
