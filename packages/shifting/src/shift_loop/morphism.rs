//! The loop scripts as Steps and Branches over the iteration states.
//!
//! Each morphism builds a new Script Bridge invocation from the state it consumes, runs it in
//! the repository root with the iteration's `SHIFT_*` variables, and hands the script's exit,
//! read through the script contract, to that state's pure transition.

use std::io;
use std::path::PathBuf;

use crate::branch::Branch;
use crate::either::Either;
use crate::repository::{Commit, GitError, Repository};
use crate::script::{Script, ScriptOutput};
use crate::shift_loop::contract::{CheckExit, PrecheckExit, SelectExit, WorkExit};
use crate::shift_loop::outcome::{SettleFailure, Settled, Settlement};
use crate::shift_loop::state::{
    AlreadyDone, Approved, Attempt, Declined, Exhausted, Idle, Rejected, Selected, Verified,
    WorkDone,
};
use crate::shift_loop::{LoopKind, LoopScriptError, ShiftLoopError};
use crate::step::Step;

const SHIFT_LOOP: &str = "SHIFT_LOOP";
const SHIFT_ITERATION: &str = "SHIFT_ITERATION";
const SHIFT_TASK: &str = "SHIFT_TASK";
const SHIFT_BASE: &str = "SHIFT_BASE";
const SHIFT_OUTCOME: &str = "SHIFT_OUTCOME";

/// The script context every script of one shift loop shares: which loop it is, and the
/// repository it runs in.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopScriptContext {
    pub kind: LoopKind,
    pub repository: Repository,
}

/// One script of a shift loop: its path, and the loop's script context it runs under.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopScript {
    pub path: PathBuf,
    pub context: LoopScriptContext,
}

impl LoopScript {
    /// The commit HEAD points at in the repository this script runs in.
    fn repository_head(&self) -> Result<Commit, GitError> {
        self.context.repository.head()
    }

    /// An invocation of this script in `iteration`, whose base commit is `base`, before select
    /// chose a task: it runs in the repository root, and the task and outcome are unset.
    fn invocation(&self, iteration: u64, base: &Commit) -> Script {
        Script::new(&self.path)
            .with_current_dir(&self.context.repository.root)
            .with_dropped_env(SHIFT_TASK)
            .with_dropped_env(SHIFT_OUTCOME)
            .with_env(SHIFT_LOOP, self.context.kind.name())
            .with_env(SHIFT_ITERATION, iteration.to_string())
            .with_env(SHIFT_BASE, &base.0)
    }

    /// An invocation of this script for `attempt`, working on its task.
    fn attempt_invocation(&self, attempt: &Attempt) -> Script {
        self.invocation(attempt.iteration, &attempt.base)
            .with_env(SHIFT_TASK, &attempt.task.0)
    }

    /// Runs `invocation`, one of this script's, with `input` on its stdin. A Script Bridge
    /// error names this script.
    fn run(&self, invocation: &Script, input: &str) -> Result<ScriptOutput, ShiftLoopError> {
        invocation
            .run(input, &mut io::stderr())
            .map_err(|error| self.error_naming_script(error))
    }

    /// Runs this script for `attempt` with its task reference as one line on stdin.
    fn run_on_task(&self, attempt: &Attempt) -> Result<ScriptOutput, ShiftLoopError> {
        self.run(
            &self.attempt_invocation(attempt),
            &format!("{}\n", attempt.task),
        )
    }

    /// Runs this commit or rollback for the settlement's attempt with its outcome and its
    /// input on stdin. A failure stops the run with the error `failed` names it by.
    fn settle(
        &self,
        settlement: Settlement,
        failed: fn(SettleFailure) -> LoopScriptError,
    ) -> Result<Settled, ShiftLoopError> {
        let invocation = self
            .attempt_invocation(&settlement.attempt)
            .with_env(SHIFT_OUTCOME, settlement.outcome.name());
        let output = self.run(&invocation, &settlement.input)?;
        settlement
            .settle(output)
            .map_err(|failure| self.error_naming_script(failed(failure)))
    }

    /// The error that stops the run while this script runs, naming the script.
    fn error_naming_script(&self, source: impl Into<LoopScriptError>) -> ShiftLoopError {
        ShiftLoopError::Script {
            script: self.path.clone(),
            source: source.into(),
        }
    }
}

/// Select: reads the base commit, then picks the iteration's task, or ends the loop.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Select {
    pub script: LoopScript,
}

impl Branch<Idle> for Select {
    type Left = Selected;
    type Right = Exhausted;
    type Error = ShiftLoopError;

    fn evaluate(&self, idle: Idle) -> Result<Either<Selected, Exhausted>, ShiftLoopError> {
        let base = self.script.repository_head()?;
        let invocation = self.script.invocation(idle.iteration, &base);
        let output = self.script.run(&invocation, "")?;
        idle.after_select(base, SelectExit::from(output))
            .map_err(|error| self.script.error_naming_script(error))
    }
}

/// Precheck: the task is already done, or it still needs work.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Precheck {
    pub script: LoopScript,
}

impl Branch<Selected> for Precheck {
    type Left = AlreadyDone;
    type Right = Selected;
    type Error = ShiftLoopError;

    fn evaluate(
        &self,
        selected: Selected,
    ) -> Result<Either<AlreadyDone, Selected>, ShiftLoopError> {
        let output = self.script.run_on_task(&selected.attempt)?;
        Ok(selected.after_precheck(PrecheckExit::from(output)))
    }
}

/// Work: done with a summary, or, on the right arm, declined or failed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Work {
    pub script: LoopScript,
}

impl Branch<Selected> for Work {
    type Left = WorkDone;
    type Right = Either<Declined, Rejected>;
    type Error = ShiftLoopError;

    fn evaluate(
        &self,
        selected: Selected,
    ) -> Result<Either<WorkDone, Either<Declined, Rejected>>, ShiftLoopError> {
        let output = self.script.run_on_task(&selected.attempt)?;
        Ok(selected.after_work(WorkExit::from(output)))
    }
}

/// Verify: checks the work summary deterministically.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Verify {
    pub script: LoopScript,
}

impl Branch<WorkDone> for Verify {
    type Left = Verified;
    type Right = Rejected;
    type Error = ShiftLoopError;

    fn evaluate(&self, done: WorkDone) -> Result<Either<Verified, Rejected>, ShiftLoopError> {
        let invocation = self.script.attempt_invocation(&done.attempt);
        let output = self.script.run(&invocation, &done.summary)?;
        Ok(done.after_verify(CheckExit::from(output)))
    }
}

/// Judge: checks the work summary after verify passed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Judge {
    pub script: LoopScript,
}

impl Branch<Verified> for Judge {
    type Left = Approved;
    type Right = Rejected;
    type Error = ShiftLoopError;

    fn evaluate(&self, verified: Verified) -> Result<Either<Approved, Rejected>, ShiftLoopError> {
        let invocation = self.script.attempt_invocation(&verified.attempt);
        let output = self.script.run(&invocation, &verified.summary)?;
        Ok(verified.after_judge(CheckExit::from(output)))
    }
}

/// Commit for `Approved` and `AlreadyDone`, rollback for `Rejected` and `Declined`, and either
/// arm of any coproduct of those.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Settle {
    pub commit: LoopScript,
    pub rollback: LoopScript,
}

impl Step<Approved> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, approved: Approved) -> Result<Settled, ShiftLoopError> {
        self.commit.settle(approved.into(), LoopScriptError::Commit)
    }
}

impl Step<AlreadyDone> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, done: AlreadyDone) -> Result<Settled, ShiftLoopError> {
        self.commit.settle(done.into(), LoopScriptError::Commit)
    }
}

impl Step<Rejected> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, rejected: Rejected) -> Result<Settled, ShiftLoopError> {
        self.rollback
            .settle(rejected.into(), LoopScriptError::Rollback)
    }
}

impl Step<Declined> for Settle {
    type Out = Settled;
    type Error = ShiftLoopError;

    fn apply(&self, declined: Declined) -> Result<Settled, ShiftLoopError> {
        self.rollback
            .settle(declined.into(), LoopScriptError::Rollback)
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
