use std::process::ExitCode;

use clap::Parser;
use shifting::cli::{self, Cli};

fn main() -> ExitCode {
    let args = Cli::parse();
    let mut stdout = std::io::stdout();
    match cli::run(args, &mut stdout) {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error: {err}");
            ExitCode::FAILURE
        }
    }
}
