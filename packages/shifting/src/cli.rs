//! The `shift` command line.

use std::io::{self, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::repository::{GitError, Repository};
use crate::shift_loop::{Idle, LoopDirectoryError, LoopKind, ShiftLoop, ShiftLoopError};
use crate::step::StepExt;

/// Command-line arguments for the `shift` binary.
#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[command(
    name = "shift",
    about = "Runs shifting's plan and build loops in a git repository",
    version
)]
pub struct Cli {
    /// Repository root, resolved through git
    #[arg(long, global = true, default_value = ".")]
    pub root: PathBuf,

    /// Maximum number of iterations; unbounded when absent
    #[arg(long, global = true)]
    pub max_iterations: Option<u64>,

    #[command(subcommand)]
    pub command: Command,
}

/// The `shift` subcommands.
#[derive(Subcommand, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// Run the plan loop from `.shifting/plan/`
    Plan,
    /// Run the build loop from `.shifting/build/`
    Build,
}

/// Errors that stop `shift`.
#[derive(thiserror::Error, Debug)]
pub enum ShiftError {
    #[error(transparent)]
    Repository(#[from] GitError),
    #[error(transparent)]
    LoopDirectory(#[from] LoopDirectoryError),
    #[error(transparent)]
    Loop(#[from] ShiftLoopError),
    #[error("failed to print the loop report: {0}")]
    Output(#[from] io::Error),
}

/// Runs the command's loop and prints its summary, then its end state as the last line.
pub fn run(cli: Cli, stdout: &mut impl Write) -> Result<(), ShiftError> {
    let repository = Repository::containing(&cli.root)?;
    let kind = match cli.command {
        Command::Plan => LoopKind::Plan,
        Command::Build => LoopKind::Build,
    };
    let report = ShiftLoop::discover(kind, repository, cli.max_iterations)?
        .into_executor()
        .run(Idle::first())?;
    writeln!(stdout, "{report}")?;
    Ok(())
}
