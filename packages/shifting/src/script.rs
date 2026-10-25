use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::branch::Branch;
use crate::either::Either;
use crate::step::Step;

/// Captured outcomes from executing a child process.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ScriptOutput {
    /// Creates a new [`ScriptOutput`] from an exit code and output streams.
    #[must_use]
    pub fn new(code: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }
}

/// Errors originating from script process execution.
#[derive(thiserror::Error, Debug)]
pub enum ScriptError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("process exited with status {code}: {stderr}")]
    NonZeroExit { code: i32, stderr: String },
    #[error("process terminated by signal")]
    Signaled,
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

fn execute_script(path: &Path, input: &str) -> Result<ScriptOutput, ScriptError> {
    let mut child = Command::new(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdin_writer = child.stdin.take().map(|mut stdin| {
        let input_bytes = input.as_bytes().to_vec();
        std::thread::spawn(move || match stdin.write_all(&input_bytes) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
            Err(e) => Err(e),
        })
    });

    let output_result = child.wait_with_output();
    let writer_result = stdin_writer.map(|handle| handle.join());

    let output = output_result?;
    if let Some(Ok(Err(io_err))) = writer_result {
        return Err(ScriptError::Io(io_err));
    }

    let code = output.status.code().ok_or(ScriptError::Signaled)?;
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;

    Ok(ScriptOutput {
        code,
        stdout,
        stderr,
    })
}

/// An unconditional state transition morphism backed by an external Unix script.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptStep {
    pub path: PathBuf,
}

impl ScriptStep {
    /// Creates a new [`ScriptStep`] pointing to the given script path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl Step<String> for ScriptStep {
    type Out = String;
    type Error = ScriptError;

    fn apply(&self, input: String) -> Result<Self::Out, Self::Error> {
        let output = execute_script(&self.path, &input)?;
        if output.code == 0 {
            Ok(output.stdout)
        } else {
            Err(ScriptError::NonZeroExit {
                code: output.code,
                stderr: output.stderr,
            })
        }
    }
}

/// A conditional decision coproduct morphism backed by an external Unix script.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptBranch {
    pub path: PathBuf,
}

impl ScriptBranch {
    /// Creates a new [`ScriptBranch`] pointing to the given script path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }
}

impl Branch<String> for ScriptBranch {
    type Left = String;
    type Right = ScriptOutput;
    type Error = ScriptError;

    fn evaluate(&self, input: String) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        let output = execute_script(&self.path, &input)?;
        if output.code == 0 {
            Ok(Either::Left(output.stdout))
        } else {
            Ok(Either::Right(output))
        }
    }
}
