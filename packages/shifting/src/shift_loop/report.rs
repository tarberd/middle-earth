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
        let count_one_more =
            |count: u64| count.checked_add(1).ok_or(CountOverflow::Outcome(outcome));
        match outcome {
            Outcome::Verified => {
                count_one_more(self.commits).map(|commits| Self { commits, ..self })
            }
            Outcome::AlreadyDone => count_one_more(self.already_done).map(|already_done| Self {
                already_done,
                ..self
            }),
            Outcome::Rejected(Rejection::WorkFailed) => {
                count_one_more(self.work_failed).map(|work_failed| Self {
                    work_failed,
                    ..self
                })
            }
            Outcome::Rejected(Rejection::VerifyFailed) => {
                count_one_more(self.verify_failed).map(|verify_failed| Self {
                    verify_failed,
                    ..self
                })
            }
            Outcome::Rejected(Rejection::JudgeFailed) => {
                count_one_more(self.judge_failed).map(|judge_failed| Self {
                    judge_failed,
                    ..self
                })
            }
            Outcome::Declined => {
                count_one_more(self.declines).map(|declines| Self { declines, ..self })
            }
        }
    }

    /// The iterations counted: each ended with exactly one outcome. Six counts of at most
    /// `u64::MAX` always add up within `u128`.
    #[must_use]
    pub fn iterations(&self) -> u128 {
        let Self {
            commits,
            already_done,
            work_failed,
            verify_failed,
            judge_failed,
            declines,
        } = *self;
        [
            commits,
            already_done,
            work_failed,
            verify_failed,
            judge_failed,
            declines,
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
        let Self {
            commits,
            already_done,
            work_failed,
            verify_failed,
            judge_failed,
            declines,
        } = self;
        writeln!(f, "iterations: {}", self.iterations())?;
        writeln!(f, "commits: {commits}")?;
        writeln!(f, "{}: {already_done}", Outcome::AlreadyDone.name())?;
        writeln!(f, "{}: {work_failed}", WORK_FAILED.name())?;
        writeln!(f, "{}: {verify_failed}", VERIFY_FAILED.name())?;
        writeln!(f, "{}: {judge_failed}", JUDGE_FAILED.name())?;
        writeln!(f, "declines: {declines}")
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
