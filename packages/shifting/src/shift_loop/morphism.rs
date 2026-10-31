//! The loop scripts as Steps and Branches over the iteration states.
//!
//! Each morphism builds a new Script Bridge invocation from the state it consumes, runs it in
//! the repository root with the iteration's `SHIFT_*` variables, and routes its exit through
//! the script contract.

use std::io;
use std::path::{Path, PathBuf};

use crate::branch::Branch;
use crate::either::Either;
use crate::repository::{Commit, Repository};
use crate::script::{Script, ScriptError, ScriptOutput};
use crate::shift_loop::contract::{CheckExit, PrecheckExit, SelectExit, WorkExit};
use crate::shift_loop::outcome::{Outcome, Rejection, Settled};
use crate::shift_loop::state::{
    AlreadyDone, Approved, Declined, Exhausted, Idle, Rejected, Selected, Task, Verified, WorkDone,
};
use crate::shift_loop::{LoopKind, ShiftLoopError};
use crate::step::Step;

const SHIFT_LOOP: &str = "SHIFT_LOOP";
const SHIFT_ITERATION: &str = "SHIFT_ITERATION";
const SHIFT_TASK: &str = "SHIFT_TASK";
const SHIFT_BASE: &str = "SHIFT_BASE";
const SHIFT_OUTCOME: &str = "SHIFT_OUTCOME";

/// What every script invocation of one loop shares.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Context {
    pub kind: LoopKind,
    pub repository: Repository,
}

impl Context {
    /// An invocation of `script` in `iteration` before select chose a task: it runs in the
    /// repository root, and the task and outcome are unset.
    fn invocation(&self, script: &Path, iteration: u64, base: &Commit) -> Script {
        Script::new(script)
            .with_current_dir(&self.repository.root)
            .without_inherited_env(SHIFT_TASK)
            .without_inherited_env(SHIFT_OUTCOME)
            .with_env(SHIFT_LOOP, self.kind.name())
            .with_env(SHIFT_ITERATION, iteration.to_string())
            .with_env(SHIFT_BASE, &base.0)
    }

    /// An invocation of `script` working on `task`.
    fn task_invocation(&self, script: &Path, iteration: u64, task: &Task, base: &Commit) -> Script {
        self.invocation(script, iteration, base)
            .with_env(SHIFT_TASK, &task.0)
    }

    /// Runs `script` on `task`, which is also its stdin.
    fn run_on_task(
        &self,
        script: &Path,
        iteration: u64,
        task: &Task,
        base: &Commit,
    ) -> Result<ScriptOutput, ScriptError> {
        self.task_invocation(script, iteration, task, base)
            .run(&format!("{task}\n"), &mut io::stderr())
    }
}

/// Select: reads the base commit, then picks the iteration's task, or ends the loop.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Select {
    pub script: PathBuf,
    pub context: Context,
}

impl Branch<Idle> for Select {
    type Left = Selected;
    type Right = Exhausted;
    type Error = ShiftLoopError;

    fn evaluate(&self, idle: Idle) -> Result<Either<Selected, Exhausted>, ShiftLoopError> {
        let base = self.context.repository.head()?;
        let output = self
            .context
            .invocation(&self.script, idle.iteration, &base)
            .run("", &mut io::stderr())?;
        match SelectExit::from(output) {
            SelectExit::Task(task) => Ok(Either::Left(Selected {
                iteration: idle.iteration,
                task,
                base,
            })),
            SelectExit::NoWorkLeft => Ok(Either::Right(Exhausted)),
            SelectExit::Failed { exit, report } => Err(ShiftLoopError::Select { exit, report }),
        }
    }
}

/// Precheck: the task is already done, or it still needs work.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Precheck {
    pub script: PathBuf,
    pub context: Context,
}

impl Branch<Selected> for Precheck {
    type Left = AlreadyDone;
    type Right = Selected;
    type Error = ShiftLoopError;

    fn evaluate(
        &self,
        selected: Selected,
    ) -> Result<Either<AlreadyDone, Selected>, ShiftLoopError> {
        let output = self.context.run_on_task(
            &self.script,
            selected.iteration,
            &selected.task,
            &selected.base,
        )?;
        Ok(match PrecheckExit::from(output) {
            PrecheckExit::AlreadyDone(output) => Either::Left(AlreadyDone {
                iteration: selected.iteration,
                task: selected.task,
                base: selected.base,
                output,
            }),
            PrecheckExit::NeedsWork => Either::Right(selected),
        })
    }
}

/// Work: done with a summary, or, on the right arm, declined or failed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Work {
    pub script: PathBuf,
    pub context: Context,
}

impl Branch<Selected> for Work {
    type Left = WorkDone;
    type Right = Either<Declined, Rejected>;
    type Error = ShiftLoopError;

    fn evaluate(
        &self,
        selected: Selected,
    ) -> Result<Either<WorkDone, Either<Declined, Rejected>>, ShiftLoopError> {
        let output = self.context.run_on_task(
            &self.script,
            selected.iteration,
            &selected.task,
            &selected.base,
        )?;
        let Selected {
            iteration,
            task,
            base,
        } = selected;
        Ok(match WorkExit::from(output) {
            WorkExit::Done(summary) => Either::Left(WorkDone {
                iteration,
                task,
                base,
                summary,
            }),
            WorkExit::Declined(reason) => Either::Right(Either::Left(Declined {
                iteration,
                task,
                base,
                reason,
            })),
            WorkExit::Failed(report) => Either::Right(Either::Right(Rejected {
                iteration,
                task,
                base,
                outcome: Rejection::WorkFailed,
                report,
            })),
        })
    }
}

/// Verify: checks the work summary deterministically.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Verify {
    pub script: PathBuf,
    pub context: Context,
}

impl Branch<WorkDone> for Verify {
    type Left = Verified;
    type Right = Rejected;
    type Error = ShiftLoopError;

    fn evaluate(&self, done: WorkDone) -> Result<Either<Verified, Rejected>, ShiftLoopError> {
        let output = self
            .context
            .task_invocation(&self.script, done.iteration, &done.task, &done.base)
            .run(&done.summary, &mut io::stderr())?;
        let WorkDone {
            iteration,
            task,
            base,
            summary,
        } = done;
        Ok(match CheckExit::from(output) {
            CheckExit::Passed(_) => Either::Left(Verified {
                iteration,
                task,
                base,
                summary,
            }),
            CheckExit::Failed(report) => Either::Right(Rejected {
                iteration,
                task,
                base,
                outcome: Rejection::VerifyFailed,
                report,
            }),
        })
    }
}

/// Judge: checks the work summary after verify passed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Judge {
    pub script: PathBuf,
    pub context: Context,
}

impl Branch<Verified> for Judge {
    type Left = Approved;
    type Right = Rejected;
    type Error = ShiftLoopError;

    fn evaluate(&self, verified: Verified) -> Result<Either<Approved, Rejected>, ShiftLoopError> {
        let output = self
            .context
            .task_invocation(
                &self.script,
                verified.iteration,
                &verified.task,
                &verified.base,
            )
            .run(&verified.summary, &mut io::stderr())?;
        let Verified {
            iteration,
            task,
            base,
            ..
        } = verified;
        Ok(match CheckExit::from(output) {
            CheckExit::Passed(verdict) => Either::Left(Approved {
                iteration,
                task,
                base,
                verdict,
            }),
            CheckExit::Failed(report) => Either::Right(Rejected {
                iteration,
                task,
                base,
                outcome: Rejection::JudgeFailed,
                report,
            }),
        })
    }
}

/// Commit for `Approved` and `AlreadyDone`, rollback for `Rejected` and `Declined`, and either
/// arm of any coproduct of those.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Settle {
    pub commit: PathBuf,
    pub rollback: PathBuf,
    pub context: Context,
}

impl Settle {
    /// Runs `script` on `task` with `outcome` and `input` on its stdin.
    fn run(
        &self,
        script: &Path,
        iteration: u64,
        task: &Task,
        base: &Commit,
        outcome: Outcome,
        input: &str,
    ) -> Result<Either<String, ScriptOutput>, ScriptError> {
        self.context
            .task_invocation(script, iteration, task, base)
            .with_env(SHIFT_OUTCOME, outcome.name())
            .run(input, &mut io::stderr())
            .map(ScriptOutput::into_success_or_failure)
    }

    fn commit(
        &self,
        iteration: u64,
        task: Task,
        base: &Commit,
        outcome: Outcome,
        input: &str,
    ) -> Result<Settled, ShiftLoopError> {
        match self.run(&self.commit, iteration, &task, base, outcome, input)? {
            Either::Left(_) => Ok(Settled { task, outcome }),
            Either::Right(failed) => Err(ShiftLoopError::Commit {
                task,
                exit: failed.exit,
                report: failed.failure_report(),
            }),
        }
    }

    fn rollback(
        &self,
        iteration: u64,
        task: Task,
        base: &Commit,
        outcome: Outcome,
        input: &str,
    ) -> Result<Settled, ShiftLoopError> {
        match self.run(&self.rollback, iteration, &task, base, outcome, input)? {
            Either::Left(_) => Ok(Settled { task, outcome }),
            Either::Right(failed) => Err(ShiftLoopError::Rollback {
                task,
                exit: failed.exit,
                report: failed.failure_report(),
            }),
        }
    }
}

impl Step<Approved> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, approved: Approved) -> Result<Settled, ShiftLoopError> {
        self.commit(
            approved.iteration,
            approved.task,
            &approved.base,
            Outcome::Verified,
            &approved.verdict,
        )
    }
}

impl Step<AlreadyDone> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, done: AlreadyDone) -> Result<Settled, ShiftLoopError> {
        self.commit(
            done.iteration,
            done.task,
            &done.base,
            Outcome::AlreadyDone,
            &done.output,
        )
    }
}

impl Step<Rejected> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, rejected: Rejected) -> Result<Settled, ShiftLoopError> {
        self.rollback(
            rejected.iteration,
            rejected.task,
            &rejected.base,
            Outcome::Rejected(rejected.outcome),
            &rejected.report,
        )
    }
}

impl Step<Declined> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, declined: Declined) -> Result<Settled, ShiftLoopError> {
        self.rollback(
            declined.iteration,
            declined.task,
            &declined.base,
            Outcome::Declined,
            &declined.reason,
        )
    }
}

/// The copairing of settling each arm: whichever state the decision reached is settled.
impl<L, R> Step<Either<L, R>> for Settle
where
    Self: Step<L, Out = Settled, Error = ShiftLoopError>
        + Step<R, Out = Settled, Error = ShiftLoopError>,
{
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, decided: Either<L, R>) -> Result<Settled, ShiftLoopError> {
        decided.either(|left| self.apply(left), |right| self.apply(right))
    }
}
