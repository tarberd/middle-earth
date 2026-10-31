//! The loop scripts as Steps and Branches over the iteration states.
//!
//! Each morphism builds a new Script Bridge invocation from the state it consumes, runs it in
//! the repository root with the iteration's `SHIFT_*` variables, and hands the script's exit,
//! read through the script contract, to that state's pure transition.

use std::io;
use std::path::{Path, PathBuf};

use crate::branch::Branch;
use crate::either::Either;
use crate::repository::{Commit, Repository};
use crate::script::{Script, ScriptError, ScriptOutput};
use crate::shift_loop::contract::{CheckExit, PrecheckExit, SelectExit, WorkExit};
use crate::shift_loop::outcome::{SettleFailure, Settled, Settlement};
use crate::shift_loop::state::{
    AlreadyDone, Approved, Attempt, Declined, Exhausted, Idle, Rejected, Selected, Verified,
    WorkDone,
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
    /// An invocation of `script` in `iteration`, whose base commit is `base`, before select
    /// chose a task: it runs in the repository root, and the task and outcome are unset.
    fn invocation(&self, script: &Path, iteration: u64, base: &Commit) -> Script {
        Script::new(script)
            .with_current_dir(&self.repository.root)
            .with_dropped_env(SHIFT_TASK)
            .with_dropped_env(SHIFT_OUTCOME)
            .with_env(SHIFT_LOOP, self.kind.name())
            .with_env(SHIFT_ITERATION, iteration.to_string())
            .with_env(SHIFT_BASE, &base.0)
    }

    /// An invocation of `script` for `attempt`, working on its task.
    fn attempt_invocation(&self, script: &Path, attempt: &Attempt) -> Script {
        self.invocation(script, attempt.iteration, &attempt.base)
            .with_env(SHIFT_TASK, &attempt.task.0)
    }

    /// Runs `script` for `attempt` with `input` on its stdin.
    fn run(
        &self,
        script: &Path,
        attempt: &Attempt,
        input: &str,
    ) -> Result<ScriptOutput, ScriptError> {
        self.attempt_invocation(script, attempt)
            .run(input, &mut io::stderr())
    }

    /// Runs `script` for `attempt` with its task reference as one line on stdin.
    fn run_on_task(&self, script: &Path, attempt: &Attempt) -> Result<ScriptOutput, ScriptError> {
        self.run(script, attempt, &format!("{}\n", attempt.task))
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
        idle.after_select(base, SelectExit::from(output))
            .map_err(ShiftLoopError::from)
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
        let output = self.context.run_on_task(&self.script, &selected.attempt)?;
        Ok(selected.after_precheck(PrecheckExit::from(output)))
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
        let output = self.context.run_on_task(&self.script, &selected.attempt)?;
        Ok(selected.after_work(WorkExit::from(output)))
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
            .run(&self.script, &done.attempt, &done.summary)?;
        Ok(done.after_verify(CheckExit::from(output)))
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
            .run(&self.script, &verified.attempt, &verified.summary)?;
        Ok(verified.after_judge(CheckExit::from(output)))
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
    /// Runs commit for `settlement`; a failure stops the run.
    fn commit(&self, settlement: Settlement) -> Result<Settled, ShiftLoopError> {
        self.run(&self.commit, settlement, ShiftLoopError::Commit)
    }

    /// Runs rollback for `settlement`; a failure stops the run.
    fn rollback(&self, settlement: Settlement) -> Result<Settled, ShiftLoopError> {
        self.run(&self.rollback, settlement, ShiftLoopError::Rollback)
    }

    /// Runs `script` for the settlement's attempt with its outcome and its input on stdin. A
    /// failure stops the run with the error `failed` names it by.
    fn run(
        &self,
        script: &Path,
        settlement: Settlement,
        failed: fn(SettleFailure) -> ShiftLoopError,
    ) -> Result<Settled, ShiftLoopError> {
        let output = self
            .context
            .attempt_invocation(script, &settlement.attempt)
            .with_env(SHIFT_OUTCOME, settlement.outcome.name())
            .run(&settlement.input, &mut io::stderr())?;
        settlement.settle(output).map_err(failed)
    }
}

impl Step<Approved> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, approved: Approved) -> Result<Settled, ShiftLoopError> {
        self.commit(approved.into())
    }
}

impl Step<AlreadyDone> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, done: AlreadyDone) -> Result<Settled, ShiftLoopError> {
        self.commit(done.into())
    }
}

impl Step<Rejected> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, rejected: Rejected) -> Result<Settled, ShiftLoopError> {
        self.rollback(rejected.into())
    }
}

impl Step<Declined> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, declined: Declined) -> Result<Settled, ShiftLoopError> {
        self.rollback(declined.into())
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
