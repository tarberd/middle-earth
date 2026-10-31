//! How an iteration that selected a task ends.

use crate::shift_loop::state::Task;

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
