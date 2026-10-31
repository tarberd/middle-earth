//! What a shift loop reports when it ends.

use core::fmt;

use crate::shift_loop::outcome::{Outcome, Rejection};
use crate::shift_loop::state::Task;

/// Counts of each iteration outcome in one loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LoopSummary {
    pub commits: u64,
    pub already_done: u64,
    pub work_failed: u64,
    pub verify_failed: u64,
    pub judge_failed: u64,
    pub declines: u64,
}

impl LoopSummary {
    /// Counts one more iteration that ended with `outcome`.
    #[must_use]
    pub const fn record(self, outcome: Outcome) -> Self {
        match outcome {
            Outcome::Verified => Self {
                commits: self.commits + 1,
                ..self
            },
            Outcome::AlreadyDone => Self {
                already_done: self.already_done + 1,
                ..self
            },
            Outcome::Rejected(Rejection::WorkFailed) => Self {
                work_failed: self.work_failed + 1,
                ..self
            },
            Outcome::Rejected(Rejection::VerifyFailed) => Self {
                verify_failed: self.verify_failed + 1,
                ..self
            },
            Outcome::Rejected(Rejection::JudgeFailed) => Self {
                judge_failed: self.judge_failed + 1,
                ..self
            },
            Outcome::Declined => Self {
                declines: self.declines + 1,
                ..self
            },
        }
    }

    /// The iterations counted: each ended with exactly one outcome.
    #[must_use]
    pub const fn iterations(&self) -> u64 {
        self.commits
            + self.already_done
            + self.work_failed
            + self.verify_failed
            + self.judge_failed
            + self.declines
    }
}

/// How a shift loop ended.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LoopEnd {
    /// Select reported no work left.
    Exhausted,
    /// Work declined `task`.
    Declined { task: Task },
    /// The iteration bound was reached.
    MaxIterations,
}

/// A loop's end state with its summary.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopReport {
    pub end: LoopEnd,
    pub summary: LoopSummary,
}

impl fmt::Display for LoopSummary {
    /// One `name: count` line per count, outcomes named as `SHIFT_OUTCOME` names them.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "iterations: {}", self.iterations())?;
        writeln!(f, "commits: {}", self.commits)?;
        writeln!(f, "already-done: {}", self.already_done)?;
        writeln!(f, "work-failed: {}", self.work_failed)?;
        writeln!(f, "verify-failed: {}", self.verify_failed)?;
        writeln!(f, "judge-failed: {}", self.judge_failed)?;
        writeln!(f, "declines: {}", self.declines)
    }
}

impl fmt::Display for LoopEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted => f.write_str("Exhausted"),
            Self::Declined { task } => write!(f, "Declined {task}"),
            Self::MaxIterations => f.write_str("MaxIterations"),
        }
    }
}

impl fmt::Display for LoopReport {
    /// The summary, then the end state as the last line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.summary, self.end)
    }
}
