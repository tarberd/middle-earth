//! The typed states of a shift loop iteration.

use core::fmt;

use crate::either::{Either, LoopFlow};
use crate::repository::Commit;
use crate::shift_loop::contract::{CheckExit, PrecheckExit, SelectError, SelectExit, WorkExit};
use crate::shift_loop::outcome::{Outcome, Rejection, Settled};
use crate::shift_loop::report::{CountOverflow, LoopEnd, LoopReport, LoopSummary};

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

    /// Routes select's exit: a task starts this iteration's attempt from `base`, no work left
    /// exhausts the loop, and anything else stops the run.
    pub fn after_select(
        self,
        base: Commit,
        exit: SelectExit,
    ) -> Result<Either<Selected, Exhausted>, SelectError> {
        match exit {
            SelectExit::Task(task) => Ok(Either::Left(Selected {
                attempt: Attempt {
                    iteration: self.iteration,
                    task,
                    base,
                },
            })),
            SelectExit::NoWorkLeft => Ok(Either::Right(Exhausted)),
            SelectExit::Stop(error) => Err(error),
        }
    }

    /// Counts how an iteration ended, then continues to the next iteration or ends the loop.
    /// A full count is an overflow, and so is numbering an iteration past the last one, which
    /// only an unbounded loop reaches.
    pub fn advance(
        self,
        ended: Either<Settled, Exhausted>,
        max_iterations: Option<u64>,
    ) -> Result<LoopFlow<Self, LoopReport>, CountOverflow> {
        match ended {
            Either::Left(Settled {
                task,
                outcome: Outcome::Declined,
            }) => self.summary.record(Outcome::Declined).map(|summary| {
                LoopFlow::Break(LoopReport {
                    end: LoopEnd::Declined { task },
                    summary,
                })
            }),
            Either::Left(Settled { outcome, .. }) => self
                .summary
                .record(outcome)
                .and_then(|summary| Self { summary, ..self }.next(max_iterations)),
            Either::Right(Exhausted) => Ok(LoopFlow::Break(LoopReport {
                end: LoopEnd::Exhausted,
                summary: self.summary,
            })),
        }
    }

    /// The loop after this iteration: idle before the next one, which the bound then checks.
    /// No bound is past the last iteration number, so a bounded loop that ran that iteration
    /// has reached its bound, and an unbounded one can't number another iteration.
    fn next(
        self,
        max_iterations: Option<u64>,
    ) -> Result<LoopFlow<Self, LoopReport>, CountOverflow> {
        match (self.iteration.checked_add(1), max_iterations) {
            (Some(iteration), _) => Ok(LoopFlow::Continue(Self { iteration, ..self })),
            (None, Some(_)) => Ok(LoopFlow::Break(LoopReport {
                end: LoopEnd::MaxIterations,
                summary: self.summary,
            })),
            (None, None) => Err(CountOverflow::Iteration),
        }
    }
}

/// Select reported no work left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Exhausted;

/// An iteration's attempt at a task: the iteration's number within the run, the task select
/// chose, and the base commit, HEAD when the iteration started.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Attempt {
    pub iteration: u64,
    pub task: Task,
    pub base: Commit,
}

/// Select chose the attempt's task.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Selected {
    pub attempt: Attempt,
}

impl Selected {
    /// Routes precheck's exit: a task already done goes to commit with precheck's stdout, and
    /// any other task still needs work.
    #[must_use]
    pub fn after_precheck(self, exit: PrecheckExit) -> Either<AlreadyDone, Self> {
        match exit {
            PrecheckExit::AlreadyDone(output) => Either::Left(AlreadyDone {
                attempt: self.attempt,
                output,
            }),
            PrecheckExit::NeedsWork => Either::Right(self),
        }
    }

    /// Routes work's exit: work done passes its summary on to verify, while a decline and a
    /// failure go to rollback.
    #[must_use]
    pub fn after_work(self, exit: WorkExit) -> Either<WorkDone, Either<Declined, Rejected>> {
        let attempt = self.attempt;
        match exit {
            WorkExit::Done(summary) => Either::Left(WorkDone { attempt, summary }),
            WorkExit::Declined(reason) => Either::Right(Either::Left(Declined { attempt, reason })),
            WorkExit::Failed(report) => Either::Right(Either::Right(Rejected {
                attempt,
                outcome: Rejection::WorkFailed,
                report,
            })),
        }
    }
}

/// Work succeeded; `summary` is its stdout.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorkDone {
    pub attempt: Attempt,
    pub summary: String,
}

impl WorkDone {
    /// Routes verify's exit: a pass hands the work summary on to judge, and a failure goes to
    /// rollback as verify failed.
    #[must_use]
    pub fn after_verify(self, exit: CheckExit) -> Either<Verified, Rejected> {
        let Self { attempt, summary } = self;
        match exit {
            CheckExit::Passed(_) => Either::Left(Verified { attempt, summary }),
            CheckExit::Failed(report) => Either::Right(Rejected {
                attempt,
                outcome: Rejection::VerifyFailed,
                report,
            }),
        }
    }
}

/// Verify passed; judge receives the work `summary`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Verified {
    pub attempt: Attempt,
    pub summary: String,
}

impl Verified {
    /// Routes judge's exit: a pass goes to commit with judge's verdict, and a failure goes to
    /// rollback as judge failed.
    #[must_use]
    pub fn after_judge(self, exit: CheckExit) -> Either<Approved, Rejected> {
        let attempt = self.attempt;
        match exit {
            CheckExit::Passed(verdict) => Either::Left(Approved { attempt, verdict }),
            CheckExit::Failed(report) => Either::Right(Rejected {
                attempt,
                outcome: Rejection::JudgeFailed,
                report,
            }),
        }
    }
}

/// Judge passed; commit receives its `verdict`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Approved {
    pub attempt: Attempt,
    pub verdict: String,
}

/// Precheck found the task done at the base commit; commit receives its `output`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AlreadyDone {
    pub attempt: Attempt,
    pub output: String,
}

/// A step failed; rollback receives its failure `report`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rejected {
    pub attempt: Attempt,
    pub outcome: Rejection,
    pub report: String,
}

/// Work declined its task; rollback receives the `reason`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Declined {
    pub attempt: Attempt,
    pub reason: String,
}
