use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = source_bootstrap::Cli::parse();
    match source_bootstrap::execute(cli) {
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
