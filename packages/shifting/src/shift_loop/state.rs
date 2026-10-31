//! The typed states of a shift loop iteration.

use core::fmt;

use crate::either::{Either, LoopFlow};
use crate::repository::Commit;
use crate::shift_loop::outcome::{Outcome, Rejection, Settled};
use crate::shift_loop::report::{LoopEnd, LoopReport, LoopSummary};

/// The task reference select chose: its trimmed stdout, which the runtime treats as opaque.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Task(pub String);

impl fmt::Display for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Between iterations: the number of the next iteration within the run, and the loop's
/// summary so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Idle {
    pub iteration: u64,
    pub summary: LoopSummary,
}

impl Idle {
    /// Before the run's first iteration.
    #[must_use]
    pub fn first() -> Self {
        Self {
            iteration: 1,
            summary: LoopSummary::default(),
        }
    }

    /// Runs the next iteration when it is within `max_iterations`, or ends the loop at the
    /// bound.
    #[must_use]
    pub const fn within(self, max_iterations: Option<u64>) -> Either<Self, LoopReport> {
        match max_iterations {
            Some(max) if self.iteration > max => Either::Right(LoopReport {
                end: LoopEnd::MaxIterations,
                summary: self.summary,
            }),
            _ => Either::Left(self),
        }
    }

    /// Counts how an iteration ended, then continues to the next iteration or ends the loop.
    #[must_use]
    pub fn advance(self, ended: Either<Settled, Exhausted>) -> LoopFlow<Self, LoopReport> {
        match ended {
            Either::Left(Settled {
                task,
                outcome: Outcome::Declined,
            }) => LoopFlow::Break(LoopReport {
                end: LoopEnd::Declined { task },
                summary: self.summary.record(Outcome::Declined),
            }),
            Either::Left(Settled { outcome, .. }) => LoopFlow::Continue(Self {
                iteration: self.iteration + 1,
                summary: self.summary.record(outcome),
            }),
            Either::Right(Exhausted) => LoopFlow::Break(LoopReport {
                end: LoopEnd::Exhausted,
                summary: self.summary,
            }),
        }
    }
}

/// Select reported no work left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Exhausted;

/// Select chose `task` in `iteration`, whose base commit is `base`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Selected {
    pub iteration: u64,
    pub task: Task,
    pub base: Commit,
}

/// Work succeeded; `summary` is its stdout.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkDone {
    pub iteration: u64,
    pub task: Task,
    pub base: Commit,
    pub summary: String,
}

/// Verify passed; judge receives the work `summary`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Verified {
    pub iteration: u64,
    pub task: Task,
    pub base: Commit,
    pub summary: String,
}

/// Judge passed; commit receives its `verdict`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Approved {
    pub iteration: u64,
    pub task: Task,
    pub base: Commit,
    pub verdict: String,
}

/// Precheck found the task done at the base commit; commit receives its `output`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AlreadyDone {
    pub iteration: u64,
    pub task: Task,
    pub base: Commit,
    pub output: String,
}

/// A step failed; rollback receives its failure `report`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rejected {
    pub iteration: u64,
    pub task: Task,
    pub base: Commit,
    pub outcome: Rejection,
    pub report: String,
}

/// Work declined its task; rollback receives the `reason`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Declined {
    pub iteration: u64,
    pub task: Task,
    pub base: Commit,
    pub reason: String,
}
