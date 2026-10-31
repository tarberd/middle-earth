//! How an iteration that selected a task ends.

use core::fmt;

use crate::script::{ScriptExit, ScriptOutput};
use crate::shift_loop::state::{AlreadyDone, Approved, Attempt, Declined, Rejected, Task};

/// Why a rollback rejected an iteration's work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rejection {
    WorkFailed,
    VerifyFailed,
    JudgeFailed,
}

/// The outcome of an iteration that selected a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// Judge passed and commit ran.
    Verified,
    /// Precheck found the task done and commit ran.
    AlreadyDone,
    /// A rollback undid the iteration's work, and the loop continues.
    Rejected(Rejection),
    /// Work declined its task; a rollback undid the iteration, and the loop ends.
    Declined,
}

impl Outcome {
    /// The outcome's name, as `SHIFT_OUTCOME` gives it to commit and rollback.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::AlreadyDone => "already-done",
            Self::Rejected(Rejection::WorkFailed) => "work-failed",
            Self::Rejected(Rejection::VerifyFailed) => "verify-failed",
            Self::Rejected(Rejection::JudgeFailed) => "judge-failed",
            Self::Declined => "declined",
        }
    }
}

/// An iteration that ran commit or rollback for its task.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Settled {
    pub task: Task,
    pub outcome: Outcome,
}

/// What commit or rollback settles: the iteration's attempt, its outcome, which the script
/// receives as `SHIFT_OUTCOME`, and the script's stdin.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Settlement {
    pub attempt: Attempt,
    pub outcome: Outcome,
    pub input: String,
}

/// A commit or rollback for `task` that ended with `exit`, leaving this failure `report`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SettleFailure {
    pub task: Task,
    pub exit: ScriptExit,
    pub report: String,
}

impl fmt::Display for SettleFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "for task {} ({}): {}", self.task, self.exit, self.report)
    }
}

impl Settlement {
    /// Routes the exit of the commit or rollback that ran for this settlement: only exit `0`
    /// settles the iteration, and anything else is a failure for its task.
    pub fn settle(self, output: ScriptOutput) -> Result<Settled, SettleFailure> {
        match output.exit {
            ScriptExit::Code(0) => Ok(Settled {
                task: self.attempt.task,
                outcome: self.outcome,
            }),
            exit => Err(SettleFailure {
                task: self.attempt.task,
                exit,
                report: output.failure_report(),
            }),
        }
    }
}

/// Commit receives judge's verdict as `verified`.
impl From<Approved> for Settlement {
    fn from(approved: Approved) -> Self {
        Self {
            attempt: approved.attempt,
            outcome: Outcome::Verified,
            input: approved.verdict,
        }
    }
}

/// Commit receives precheck's stdout as `already-done`.
impl From<AlreadyDone> for Settlement {
    fn from(done: AlreadyDone) -> Self {
        Self {
            attempt: done.attempt,
            outcome: Outcome::AlreadyDone,
            input: done.output,
        }
    }
}

/// Rollback receives the failing step's report under its rejection.
impl From<Rejected> for Settlement {
    fn from(rejected: Rejected) -> Self {
        Self {
            attempt: rejected.attempt,
            outcome: Outcome::Rejected(rejected.outcome),
            input: rejected.report,
        }
    }
}

/// Rollback receives work's reason as `declined`.
impl From<Declined> for Settlement {
    fn from(declined: Declined) -> Self {
        Self {
            attempt: declined.attempt,
            outcome: Outcome::Declined,
            input: declined.reason,
        }
    }
}
