//! The shift loop: the plan and build loops, whose iterations run select, precheck, work,
//! verify and judge, then commit or rollback.

pub mod contract;
pub mod morphism;
pub mod outcome;
pub mod report;
pub mod state;

use std::path::{Path, PathBuf};

use crate::branch::{Branch, BranchExt, BranchLeft, BranchRight};
use crate::either::{Either, LoopFlow};
use crate::repository::{GitError, Repository};
use crate::script::{ScriptError, ScriptExit};
use crate::step::Step;

pub use contract::{CheckExit, PrecheckExit, SelectExit, WorkExit};
pub use morphism::{Context, Judge, Precheck, Select, Settle, Verify, Work};
pub use outcome::{Outcome, Rejection, Settled};
pub use report::{LoopEnd, LoopReport, LoopSummary};
pub use state::{
    AlreadyDone, Approved, Declined, Exhausted, Idle, Rejected, Selected, Task, Verified, WorkDone,
};

/// Which shift loop runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoopKind {
    Plan,
    Build,
}

impl LoopKind {
    /// The loop's name: its `SHIFT_LOOP` value and its directory under `.shifting/`.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Build => "build",
        }
    }
}

/// A loop directory's seven scripts.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LoopScripts {
    pub select: PathBuf,
    pub precheck: PathBuf,
    pub work: PathBuf,
    pub verify: PathBuf,
    pub judge: PathBuf,
    pub commit: PathBuf,
    pub rollback: PathBuf,
}

/// A loop directory that can't run.
#[derive(thiserror::Error, Debug)]
pub enum LoopDirectoryError {
    #[error("loop directory not found: {}", .0.display())]
    MissingDirectory(PathBuf),
    #[error("required script not found: {}", .0.display())]
    MissingScript(PathBuf),
}

impl LoopScripts {
    /// Finds every required script in `dir`, reporting the directory or the first missing
    /// script by its path.
    pub fn discover(dir: &Path) -> Result<Self, LoopDirectoryError> {
        let script = |name: &str| {
            let path = dir.join(name);
            if path.is_file() {
                Ok(path)
            } else {
                Err(LoopDirectoryError::MissingScript(path))
            }
        };
        if dir.is_dir() {
            Ok(Self {
                select: script("select.sh")?,
                precheck: script("precheck.sh")?,
                work: script("work.sh")?,
                verify: script("verify.sh")?,
                judge: script("judge.sh")?,
                commit: script("commit.sh")?,
                rollback: script("rollback.sh")?,
            })
        } else {
            Err(LoopDirectoryError::MissingDirectory(dir.to_path_buf()))
        }
    }
}

/// Errors that stop a shift loop.
#[derive(thiserror::Error, Debug)]
pub enum ShiftLoopError {
    #[error(transparent)]
    Script(#[from] ScriptError),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error("select failed ({exit}): {report}")]
    Select { exit: ScriptExit, report: String },
    #[error("commit failed for task {task} ({exit}): {report}")]
    Commit {
        task: Task,
        exit: ScriptExit,
        report: String,
    },
    #[error("rollback failed for task {task} ({exit}): {report}")]
    Rollback {
        task: Task,
        exit: ScriptExit,
        report: String,
    },
}

/// The errors of composed morphisms nest in [`Either`]; each arm is a [`ShiftLoopError`].
impl<L, R> From<Either<L, R>> for ShiftLoopError
where
    Self: From<L> + From<R>,
{
    fn from(error: Either<L, R>) -> Self {
        error.either(Self::from, Self::from)
    }
}

/// Select, then precheck on its left arm, then work on precheck's right arm, then verify
/// cascading into judge on work's left arm.
pub type Decide =
    BranchLeft<Select, BranchRight<Precheck, BranchLeft<Work, BranchLeft<Verify, Judge>>>>;

/// One shift loop iteration as a Step over [`Idle`], driven by an
/// [`Executor`](crate::executor::Executor) until the loop ends.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShiftLoop {
    pub decide: Decide,
    pub settle: Settle,
    pub max_iterations: Option<u64>,
}

impl ShiftLoop {
    /// Composes the loop from `scripts`.
    #[must_use]
    pub fn new(context: Context, scripts: LoopScripts, max_iterations: Option<u64>) -> Self {
        let LoopScripts {
            select,
            precheck,
            work,
            verify,
            judge,
            commit,
            rollback,
        } = scripts;
        let decide = Select {
            script: select,
            context: context.clone(),
        }
        .branch_left(
            Precheck {
                script: precheck,
                context: context.clone(),
            }
            .branch_right(
                Work {
                    script: work,
                    context: context.clone(),
                }
                .branch_left(
                    Verify {
                        script: verify,
                        context: context.clone(),
                    }
                    .branch_left(Judge {
                        script: judge,
                        context: context.clone(),
                    }),
                ),
            ),
        );
        Self {
            decide,
            settle: Settle {
                commit,
                rollback,
                context,
            },
            max_iterations,
        }
    }

    /// The `kind` loop of `repository`, from its directory `.shifting/<kind>/`.
    pub fn discover(
        kind: LoopKind,
        repository: Repository,
        max_iterations: Option<u64>,
    ) -> Result<Self, LoopDirectoryError> {
        LoopScripts::discover(&repository.root.join(".shifting").join(kind.name()))
            .map(|scripts| Self::new(Context { kind, repository }, scripts, max_iterations))
    }

    /// Decides the iteration from select to judge, then settles it with commit or rollback.
    fn iterate(&self, idle: Idle) -> Result<Either<Settled, Exhausted>, ShiftLoopError> {
        match self.decide.evaluate(idle)? {
            Either::Left(decided) => self.settle.apply(decided).map(Either::Left),
            Either::Right(exhausted) => Ok(Either::Right(exhausted)),
        }
    }
}

impl Step<Idle> for ShiftLoop {
    type Out = LoopFlow<Idle, LoopReport>;
    type Error = ShiftLoopError;

    fn apply(&self, idle: Idle) -> Result<LoopFlow<Idle, LoopReport>, ShiftLoopError> {
        idle.within(self.max_iterations).either(
            |idle| self.iterate(idle).map(|ended| idle.advance(ended)),
            |report| Ok(LoopFlow::Break(report)),
        )
    }
}
