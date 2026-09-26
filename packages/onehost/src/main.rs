use clap::Parser;
use onehost::cli::{self, Cli};
use onehost::hypervisor::VirshHypervisor;
use onehost::process::SystemProcessRunner;
use onehost::storage::QemuImgStorage;
use std::process::ExitCode;
use tracing_subscriber::EnvFilter;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let cli_arguments = Cli::parse();
    let hypervisor = VirshHypervisor::new();
    let storage = QemuImgStorage::new();
    let process_runner = SystemProcessRunner::new();
    let mut standard_output = std::io::stdout();

    match cli::dispatch(
        cli_arguments,
        &hypervisor,
        &storage,
        &process_runner,
        &mut standard_output,
    ) {
        Ok(exit_code) => match u8::try_from(exit_code) {
            Ok(code_u8) => ExitCode::from(code_u8),
            Err(_) => ExitCode::FAILURE,
        },
        Err(cli_error) => {
            tracing::error!("{cli_error}");
            ExitCode::FAILURE
        }
    }
}
