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

/// A count of the loop that is already at its maximum and can't count one more.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CountOverflow {
    #[error("the loop can't count another {} iteration", .0.name())]
    Outcome(Outcome),
    #[error("the iteration number can't go past {}", u64::MAX)]
    Iteration,
}

impl LoopSummary {
    /// Counts one more iteration that ended with `outcome`, or reports that its count is full.
    pub fn record(self, outcome: Outcome) -> Result<Self, CountOverflow> {
        let more = |count: u64| count.checked_add(1).ok_or(CountOverflow::Outcome(outcome));
        match outcome {
            Outcome::Verified => more(self.commits).map(|commits| Self { commits, ..self }),
            Outcome::AlreadyDone => more(self.already_done).map(|already_done| Self {
                already_done,
                ..self
            }),
            Outcome::Rejected(Rejection::WorkFailed) => {
                more(self.work_failed).map(|work_failed| Self {
                    work_failed,
                    ..self
                })
            }
            Outcome::Rejected(Rejection::VerifyFailed) => {
                more(self.verify_failed).map(|verify_failed| Self {
                    verify_failed,
                    ..self
                })
            }
            Outcome::Rejected(Rejection::JudgeFailed) => {
                more(self.judge_failed).map(|judge_failed| Self {
                    judge_failed,
                    ..self
                })
            }
            Outcome::Declined => more(self.declines).map(|declines| Self { declines, ..self }),
        }
    }

    /// The iterations counted: each ended with exactly one outcome. Six counts of at most
    /// `u64::MAX` always add up within `u128`.
    #[must_use]
    pub fn iterations(&self) -> u128 {
        [
            self.commits,
            self.already_done,
            self.work_failed,
            self.verify_failed,
            self.judge_failed,
            self.declines,
        ]
        .into_iter()
        .map(u128::from)
        .sum()
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

const WORK_FAILED: Outcome = Outcome::Rejected(Rejection::WorkFailed);
const VERIFY_FAILED: Outcome = Outcome::Rejected(Rejection::VerifyFailed);
const JUDGE_FAILED: Outcome = Outcome::Rejected(Rejection::JudgeFailed);

impl fmt::Display for LoopSummary {
    /// One `name: count` line per count: the iterations, the commits, each outcome other than
    /// a commit or a decline by its `SHIFT_OUTCOME` name, then the declines.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "iterations: {}", self.iterations())?;
        writeln!(f, "commits: {}", self.commits)?;
        writeln!(f, "{}: {}", Outcome::AlreadyDone.name(), self.already_done)?;
        writeln!(f, "{}: {}", WORK_FAILED.name(), self.work_failed)?;
        writeln!(f, "{}: {}", VERIFY_FAILED.name(), self.verify_failed)?;
        writeln!(f, "{}: {}", JUDGE_FAILED.name(), self.judge_failed)?;
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
