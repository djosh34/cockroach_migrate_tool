use clap::Parser;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = runner::Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to start async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(runner::execute(cli)) {
        Ok(Some(output)) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
