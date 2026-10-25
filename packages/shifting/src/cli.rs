use std::io::Write;
use std::path::PathBuf;

use clap::Parser;

use crate::default_loop::{DefaultShiftError, DefaultShiftLoop, Idle, ShiftSummary};
use crate::either::LoopFlow;
use crate::executor::Executor;
use crate::step::Step;

/// Command-line arguments for the `shift` binary.
#[derive(Parser, Debug, Clone, PartialEq, Eq)]
#[command(
    name = "shift",
    about = "Category-Theory Type-State Machine Agentic Orchestrator",
    version
)]
pub struct Cli {
    /// Directory containing shifting scripts (plan.sh, work.sh, verify.sh, commit.sh, rollback.sh)
    #[arg(short, long, default_value = ".shifting")]
    pub dir: PathBuf,

    /// Maximum number of iterations to execute
    #[arg(short, long)]
    pub max_iterations: Option<usize>,

    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VerboseLoop<S> {
    inner: S,
}

impl<S> Step<Idle> for VerboseLoop<S>
where
    S: Step<Idle, Out = LoopFlow<Idle, ShiftSummary>>,
{
    type Out = LoopFlow<Idle, ShiftSummary>;
    type Error = S::Error;

    fn apply(&self, input: Idle) -> Result<Self::Out, Self::Error> {
        eprintln!("[shift] iteration {}: planning...", input.iteration);
        match self.inner.apply(input)? {
            LoopFlow::Continue(next) => {
                eprintln!(
                    "[shift] iteration {}: completed cycle, continuing to iteration {}.",
                    input.iteration, next.iteration
                );
                Ok(LoopFlow::Continue(next))
            }
            LoopFlow::Break(summary) => {
                eprintln!(
                    "[shift] iteration {}: loop terminated cleanly ({} total iterations).",
                    input.iteration, summary.iterations
                );
                Ok(LoopFlow::Break(summary))
            }
        }
    }
}

/// Executes the CLI dispatch flow with standard output stream injection.
pub fn run(cli: Cli, stdout: &mut impl Write) -> Result<ShiftSummary, DefaultShiftError> {
    let shift_loop = DefaultShiftLoop::discover(&cli.dir)?
        .with_max_iterations(cli.max_iterations);

    let summary = if cli.verbose {
        Executor::new(VerboseLoop { inner: shift_loop }).run(Idle::default())?
    } else {
        Executor::new(shift_loop).run(Idle::default())?
    };

    writeln!(stdout, "Shift completed successfully ({} iterations).", summary.iterations)?;

    Ok(summary)
}
