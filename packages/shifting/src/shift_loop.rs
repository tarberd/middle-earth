//! The shift loop: the plan and build loops, whose iterations run select, precheck, work,
//! verify and judge, then commit or rollback.

pub mod contract;
pub mod morphism;
pub mod outcome;
pub mod report;
pub mod state;

use std::fs::{self, FileType};
use std::io;
use std::path::{Path, PathBuf};

use crate::branch::{Branch, BranchExt, BranchLeft, BranchRight};
use crate::either::{Either, LoopFlow};
use crate::repository::{GitError, Repository};
use crate::script::ScriptError;
use crate::step::Step;

pub use contract::{CheckExit, PrecheckExit, SelectError, SelectExit, WorkExit};
pub use morphism::{Judge, LoopScript, LoopScriptContext, Precheck, Select, Settle, Verify, Work};
pub use outcome::{Outcome, Rejection, SettleFailure, Settled, Settlement};
pub use report::{CountOverflow, LoopEnd, LoopReport, LoopSummary};
pub use state::{
    AlreadyDone, Approved, Attempt, Declined, Exhausted, Idle, Rejected, Selected, Task, Verified,
    WorkDone,
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

/// The paths of a loop directory's seven scripts.
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
    #[error("cannot inspect {}: {source}", .path.display())]
    Uninspectable {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl LoopScripts {
    /// Finds every required script in `dir`, reporting the directory or the first missing
    /// script by its path. A path the file system can't inspect is reported with its cause.
    pub fn discover(dir: &Path) -> Result<Self, LoopDirectoryError> {
        let script = |name: &str| {
            let path = dir.join(name);
            match inspect(&path)? {
                Some(file_type) if file_type.is_file() => Ok(path),
                _ => Err(LoopDirectoryError::MissingScript(path)),
            }
        };
        match inspect(dir)? {
            Some(file_type) if file_type.is_dir() => Ok(Self {
                select: script("select.sh")?,
                precheck: script("precheck.sh")?,
                work: script("work.sh")?,
                verify: script("verify.sh")?,
                judge: script("judge.sh")?,
                commit: script("commit.sh")?,
                rollback: script("rollback.sh")?,
            }),
            _ => Err(LoopDirectoryError::MissingDirectory(dir.to_path_buf())),
        }
    }
}

/// What `path` is, following symbolic links, or `None` when the file system reports that it
/// doesn't exist.
fn inspect(path: &Path) -> Result<Option<FileType>, LoopDirectoryError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(Some(metadata.file_type())),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(LoopDirectoryError::Uninspectable {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Errors that stop a shift loop.
#[derive(thiserror::Error, Debug)]
pub enum ShiftLoopError {
    /// An error raised while running the loop script at `script`, which it names.
    #[error("{}: {source}", .script.display())]
    Script {
        script: PathBuf,
        #[source]
        source: LoopScriptError,
    },
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Overflow(#[from] CountOverflow),
}

/// What stops a shift loop while one of its scripts runs.
#[derive(thiserror::Error, Debug)]
pub enum LoopScriptError {
    #[error(transparent)]
    Bridge(#[from] ScriptError),
    #[error(transparent)]
    Select(#[from] SelectError),
    #[error("commit failed {0}")]
    Commit(SettleFailure),
    #[error("rollback failed {0}")]
    Rollback(SettleFailure),
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
    /// Composes the loop from `scripts`, each running under `context`.
    #[must_use]
    pub fn new(
        context: &LoopScriptContext,
        scripts: LoopScripts,
        max_iterations: Option<u64>,
    ) -> Self {
        let LoopScripts {
            select,
            precheck,
            work,
            verify,
            judge,
            commit,
            rollback,
        } = scripts;
        let script = |path| LoopScript {
            path,
            context: context.clone(),
        };
        let decide = Select {
            script: script(select),
        }
        .branch_left(
            Precheck {
                script: script(precheck),
            }
            .branch_right(
                Work {
                    script: script(work),
                }
                .branch_left(
                    Verify {
                        script: script(verify),
                    }
                    .branch_left(Judge {
                        script: script(judge),
                    }),
                ),
            ),
        );
        Self {
            decide,
            settle: Settle {
                commit: script(commit),
                rollback: script(rollback),
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
        LoopScripts::discover(&repository.root.join(".shifting").join(kind.name())).map(|scripts| {
            Self::new(
                &LoopScriptContext { kind, repository },
                scripts,
                max_iterations,
            )
        })
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
            |idle| {
                self.iterate(idle).and_then(|ended| {
                    idle.advance(ended, self.max_iterations)
                        .map_err(ShiftLoopError::from)
                })
            },
            |report| Ok(LoopFlow::Break(report)),
        )
    }
}
