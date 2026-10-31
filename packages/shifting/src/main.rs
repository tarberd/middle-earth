use std::process::ExitCode;

use clap::Parser;
use shifting::cli::{self, Cli};

fn main() -> ExitCode {
    match cli::run(Cli::parse(), &mut std::io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error}");
            ExitCode::FAILURE
        }
    }
}
