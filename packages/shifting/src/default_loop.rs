use std::path::PathBuf;

use crate::branch::Branch;
use crate::either::{Either, LoopFlow};
use crate::executor::Executor;
use crate::script::{ScriptBranch, ScriptError, ScriptOutput, ScriptStep};
use crate::step::{AndThen, Step, StepExt};

const REQUIRED_SCRIPTS: [&str; 5] = [
    "plan.sh",
    "work.sh",
    "verify.sh",
    "commit.sh",
    "rollback.sh",
];

/// Represents the idle state between iterative execution cycles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Idle {
    pub iteration: usize,
}

impl Idle {
    /// Creates a new [`Idle`] state with the specified iteration index.
    #[must_use]
    pub const fn new(iteration: usize) -> Self {
        Self { iteration }
    }
}

impl From<usize> for Idle {
    fn from(iteration: usize) -> Self {
        Self::new(iteration)
    }
}

/// Metadata summarizing the outcome of a completed shifting run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct ShiftSummary {
    pub iterations: usize,
}

impl ShiftSummary {
    /// Creates a new [`ShiftSummary`] with the completed iteration count.
    #[must_use]
    pub const fn new(iterations: usize) -> Self {
        Self { iterations }
    }
}

/// Errors originating from default shift loop execution or script discovery.
#[derive(thiserror::Error, Debug)]
pub enum DefaultShiftError {
    #[error("directory not found: {0}")]
    DirectoryNotFound(PathBuf),
    #[error("required script not found: {0}")]
    MissingScript(PathBuf),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("script error: {0}")]
    Script(#[from] ScriptError),
}

/// A categorical decision morphism for `plan.sh`.
///
/// Maps exit code 0 to `Either::Left(stdout)` for downstream processing by `work.sh`,
/// exit code 2 directly to `Either::Right(LoopFlow::Break(()))` signaling clean backlog exhaustion,
/// and other non-zero exit codes to `Err(ScriptError::NonZeroExit)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PlanBranch {
    pub path: PathBuf,
}

impl PlanBranch {
    /// Creates a new [`PlanBranch`] pointing to the given script path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl Branch<String> for PlanBranch {
    type Left = String;
    type Right = LoopFlow<Idle, ()>;
    type Error = ScriptError;

    fn evaluate(&self, input: String) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        let branch = ScriptBranch::new(&self.path);
        match branch.evaluate(input)? {
            Either::Left(stdout) => Ok(Either::Left(stdout)),
            Either::Right(output) if output.code == 2 => Ok(Either::Right(LoopFlow::Break(()))),
            Either::Right(output) => Err(ScriptError::NonZeroExit {
                code: output.code,
                stderr: output.stderr,
            }),
        }
    }
}

impl Branch<Idle> for PlanBranch {
    type Left = String;
    type Right = LoopFlow<Idle, ()>;
    type Error = ScriptError;

    fn evaluate(&self, input: Idle) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        <Self as Branch<String>>::evaluate(self, input.iteration.to_string())
    }
}

/// Resolves verification output by applying `commit.sh` on success or `rollback.sh` on failure.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VerifyResolution {
    pub commit: ScriptStep,
    pub rollback: ScriptStep,
}

impl VerifyResolution {
    /// Creates a new [`VerifyResolution`] with the given commit and rollback steps.
    #[must_use]
    pub const fn new(commit: ScriptStep, rollback: ScriptStep) -> Self {
        Self { commit, rollback }
    }
}

impl Step<Either<String, ScriptOutput>> for VerifyResolution {
    type Out = String;
    type Error = ScriptError;

    fn apply(&self, input: Either<String, ScriptOutput>) -> Result<Self::Out, Self::Error> {
        input.either(
            |verified_stdout| self.commit.apply(verified_stdout),
            |rejected_output| {
                let rollback_input = if rejected_output.stderr.is_empty() {
                    rejected_output.stdout
                } else {
                    rejected_output.stderr
                };
                self.rollback.apply(rollback_input)
            },
        )
    }
}

/// Composes `verify.sh` decision branching with `VerifyResolution` execution.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VerifyAndResolve {
    pub verify: ScriptBranch,
    pub resolution: VerifyResolution,
}

impl VerifyAndResolve {
    /// Creates a new [`VerifyAndResolve`] step.
    #[must_use]
    pub const fn new(verify: ScriptBranch, commit: ScriptStep, rollback: ScriptStep) -> Self {
        Self {
            verify,
            resolution: VerifyResolution::new(commit, rollback),
        }
    }
}

impl Step<String> for VerifyAndResolve {
    type Out = String;
    type Error = ScriptError;

    fn apply(&self, input: String) -> Result<Self::Out, Self::Error> {
        let decision = self.verify.evaluate(input)?;
        self.resolution.apply(decision)
    }
}

/// Default orchestrator pipeline wiring the canonical 5-script convention into a Category Theory loop.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefaultShiftLoop {
    pub plan: PlanBranch,
    pub cycle: AndThen<ScriptStep, VerifyAndResolve>,
    pub max_iterations: Option<usize>,
}

impl DefaultShiftLoop {
    /// Creates a new [`DefaultShiftLoop`] for the given directory without validating disk state.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let plan = PlanBranch::new(dir.join("plan.sh"));
        let work = ScriptStep::new(dir.join("work.sh"));
        let verify = ScriptBranch::new(dir.join("verify.sh"));
        let commit = ScriptStep::new(dir.join("commit.sh"));
        let rollback = ScriptStep::new(dir.join("rollback.sh"));

        let cycle = work.and_then(VerifyAndResolve::new(verify, commit, rollback));

        Self {
            plan,
            cycle,
            max_iterations: None,
        }
    }

    /// Discovers and validates that the directory and all 5 canonical scripts exist.
    pub fn discover(dir: impl Into<PathBuf>) -> Result<Self, DefaultShiftError> {
        let dir = dir.into();
        if !dir.is_dir() {
            return Err(DefaultShiftError::DirectoryNotFound(dir));
        }

        REQUIRED_SCRIPTS
            .into_iter()
            .map(|name| dir.join(name))
            .try_fold((), |(), path| {
                if path.is_file() {
                    Ok(())
                } else {
                    Err(DefaultShiftError::MissingScript(path))
                }
            })?;

        Ok(Self::new(dir))
    }

    /// Configures an optional maximum iteration bound.
    #[must_use]
    pub fn with_max_iterations(self, max_iterations: Option<usize>) -> Self {
        Self {
            max_iterations,
            ..self
        }
    }

    /// Runs the default shift loop iteratively via [`Executor`] until clean completion or error.
    pub fn run(self) -> Result<ShiftSummary, DefaultShiftError> {
        let executor = Executor::new(self);
        executor.run(Idle::default())
    }
}

impl Step<Idle> for DefaultShiftLoop {
    type Out = LoopFlow<Idle, ShiftSummary>;
    type Error = DefaultShiftError;

    fn apply(&self, input: Idle) -> Result<Self::Out, Self::Error> {
        match self.max_iterations {
            Some(max) if input.iteration >= max => Ok(LoopFlow::Break(ShiftSummary {
                iterations: input.iteration,
            })),
            _ => self.plan.evaluate(input)?.either(
                |work_input| {
                    self.cycle
                        .apply(work_input)
                        .map_err(Either::into_inner)?;
                    Ok(LoopFlow::Continue(Idle {
                        iteration: input.iteration + 1,
                    }))
                },
                |break_flow| {
                    let summary = ShiftSummary {
                        iterations: input.iteration,
                    };
                    Ok(match break_flow {
                        LoopFlow::Break(()) => LoopFlow::Break(summary),
                        LoopFlow::Continue(idle) => LoopFlow::Continue(idle),
                    })
                },
            ),
        }
    }
}
