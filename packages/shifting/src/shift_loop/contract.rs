//! The script contract: what each loop script's exit status means.

use crate::script::{ScriptExit, ScriptOutput};
use crate::shift_loop::state::Task;

/// What a run of `select.sh` means.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SelectExit {
    /// Exit `0`: its trimmed stdout is the task reference.
    Task(Task),
    /// Exit `2`: no work is left.
    NoWorkLeft,
    /// Any other end: the run stops with this exit and select's failure report.
    Failed { exit: ScriptExit, report: String },
}

impl From<ScriptOutput> for SelectExit {
    fn from(output: ScriptOutput) -> Self {
        match output.exit {
            ScriptExit::Code(0) => Self::Task(Task(output.stdout.trim().to_string())),
            ScriptExit::Code(2) => Self::NoWorkLeft,
            exit => Self::Failed {
                exit,
                report: output.failure_report(),
            },
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
        match output.exit {
            ScriptExit::Code(0) => Self::AlreadyDone(output.stdout),
            _ => Self::NeedsWork,
        }
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
        match output.exit {
            ScriptExit::Code(0) => Self::Passed(output.stdout),
            _ => Self::Failed(output.failure_report()),
        }
    }
}
