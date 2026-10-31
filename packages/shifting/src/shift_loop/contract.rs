//! The script contract: what each loop script's exit status means.

use crate::script::{ScriptExit, ScriptOutput};
use crate::shift_loop::state::Task;

/// What a run of `select.sh` means.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SelectExit {
    /// Exit `0`: its trimmed stdout, one line, is the task reference.
    Task(Task),
    /// Exit `2`: no work is left.
    NoWorkLeft,
    /// Any other end, or a task reference spanning several lines: the run stops.
    Stop(SelectError),
}

/// Why a select stops the run.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq, Hash)]
pub enum SelectError {
    /// Select ended with `exit`, other than `0` or `2`, leaving this failure report.
    #[error("select failed ({exit}): {report}")]
    Failed { exit: ScriptExit, report: String },
    /// Select exited `0`, but its trimmed stdout spans several lines.
    #[error("select chose a task reference that spans several lines: {0:?}")]
    SeveralLines(String),
}

impl From<ScriptOutput> for SelectExit {
    fn from(output: ScriptOutput) -> Self {
        match output.exit {
            ScriptExit::Code(0) => {
                let reference = output.stdout.trim();
                if reference.contains(['\n', '\r']) {
                    Self::Stop(SelectError::SeveralLines(reference.to_string()))
                } else {
                    Self::Task(Task(reference.to_string()))
                }
            }
            ScriptExit::Code(2) => Self::NoWorkLeft,
            exit => Self::Stop(SelectError::Failed {
                exit,
                report: output.failure_report(),
            }),
        }
    }
}

/// What a run of `precheck.sh` means.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrecheckExit {
    /// Exit `0`: the task is already done; its stdout goes to commit.
    AlreadyDone(String),
    /// Any other end, a kill by signal included: the task needs work.
    NeedsWork,
}

impl From<ScriptOutput> for PrecheckExit {
    fn from(output: ScriptOutput) -> Self {
        output
            .into_success_or_failure()
            .either(Self::AlreadyDone, |_| Self::NeedsWork)
    }
}

/// What a run of `work.sh` means.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WorkExit {
    /// Exit `0`: its stdout is the work summary.
    Done(String),
    /// Exit `3`: the task is wrong; its stdout is the reason.
    Declined(String),
    /// Any other end, a kill by signal included: work failed with this failure report.
    Failed(String),
}

impl From<ScriptOutput> for WorkExit {
    fn from(output: ScriptOutput) -> Self {
        match output.exit {
            ScriptExit::Code(0) => Self::Done(output.stdout),
            ScriptExit::Code(3) => Self::Declined(output.stdout),
            _ => Self::Failed(output.failure_report()),
        }
    }
}

/// What a run of `verify.sh` or `judge.sh` means.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CheckExit {
    /// Exit `0`: the check passed with this stdout.
    Passed(String),
    /// Any other end, a kill by signal included: the check failed with this failure report.
    Failed(String),
}

impl From<ScriptOutput> for CheckExit {
    fn from(output: ScriptOutput) -> Self {
        output
            .into_success_or_failure()
            .either(Self::Passed, |failed| Self::Failed(failed.failure_report()))
    }
}
