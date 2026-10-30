use std::ffi::OsString;
use std::fmt;
use std::io::{self, Read, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::thread::ScopedJoinHandle;

use crate::branch::Branch;
use crate::either::Either;
use crate::step::Step;

/// How a script run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScriptExit {
    /// The script exited with this status code; only `0` is a success.
    Code(i32),
    /// The script was killed by this signal.
    Signal(i32),
    /// The script wrote stdout that is not valid UTF-8, which counts as the script failing.
    InvalidStdout,
}

impl ScriptExit {
    fn of(status: ExitStatus) -> io::Result<Self> {
        status
            .code()
            .map(Self::Code)
            .or_else(|| status.signal().map(Self::Signal))
            .ok_or_else(|| {
                io::Error::other(format!(
                    "script ended without an exit code or a signal: {status}"
                ))
            })
    }

    /// The exit of a script whose stdout is not valid UTF-8: a kill by signal still names its
    /// signal, any other exit becomes [`ScriptExit::InvalidStdout`].
    const fn with_invalid_stdout(self) -> Self {
        match self {
            Self::Signal(_) => self,
            Self::Code(_) | Self::InvalidStdout => Self::InvalidStdout,
        }
    }
}

impl fmt::Display for ScriptExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Code(code) => write!(f, "exit status: {code}"),
            // A raw wait status equal to a signal number encodes a kill by that signal,
            // which std displays with the signal's name, e.g. `signal: 9 (SIGKILL)`.
            Self::Signal(signal) => write!(f, "{}", ExitStatus::from_raw(*signal)),
            Self::InvalidStdout => write!(f, "invalid UTF-8 on stdout"),
        }
    }
}

/// Captured outcomes from executing a child process.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptOutput {
    pub exit: ScriptExit,
    pub stdout: String,
    pub stderr: String,
}

impl ScriptOutput {
    /// Creates a new [`ScriptOutput`] from how the script ended and its output streams.
    #[must_use]
    pub fn new(exit: ScriptExit, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            exit,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    /// Decodes captured streams: stdout that is not valid UTF-8 fails the script, while stderr,
    /// which is for humans, is decoded lossily.
    fn decode(exit: ScriptExit, stdout: Vec<u8>, stderr: &[u8]) -> Self {
        let stderr = String::from_utf8_lossy(stderr).into_owned();
        match String::from_utf8(stdout) {
            Ok(stdout) => Self {
                exit,
                stdout,
                stderr,
            },
            Err(invalid) => Self {
                exit: exit.with_invalid_stdout(),
                stdout: String::from_utf8_lossy(invalid.as_bytes()).into_owned(),
                stderr,
            },
        }
    }

    /// The report a failed script leaves behind: its stderr, or its stdout when stderr is empty.
    #[must_use]
    pub fn failure_report(self) -> String {
        if self.stderr.is_empty() {
            self.stdout
        } else {
            self.stderr
        }
    }
}

/// Errors originating from script process execution.
#[derive(thiserror::Error, Debug)]
pub enum ScriptError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("script failed ({exit}): {stderr}")]
    Failed { exit: ScriptExit, stderr: String },
}

/// An executable script invocation for the Script Bridge.
///
/// The script inherits shift's environment, plus the variables in `env`, and runs in
/// `current_dir`, or in shift's working directory when it is `None`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Script {
    pub path: PathBuf,
    pub env: Vec<(OsString, OsString)>,
    pub current_dir: Option<PathBuf>,
}

impl Script {
    /// Creates a new [`Script`] invocation of the executable at `path`.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            env: Vec::new(),
            current_dir: None,
        }
    }

    /// Adds an environment variable for this invocation.
    #[must_use]
    pub fn with_env(self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        Self {
            env: self
                .env
                .into_iter()
                .chain(std::iter::once((key.into(), value.into())))
                .collect(),
            ..self
        }
    }

    /// Sets the working directory for this invocation.
    #[must_use]
    pub fn with_current_dir(self, dir: impl Into<PathBuf>) -> Self {
        Self {
            current_dir: Some(dir.into()),
            ..self
        }
    }

    /// Runs the script with `input` on its stdin, capturing stdout and stderr.
    ///
    /// Stderr is copied to `stderr_sink` as the script writes it.
    pub fn run(
        &self,
        input: &str,
        stderr_sink: &mut impl Write,
    ) -> Result<ScriptOutput, ScriptError> {
        let mut command = Command::new(&self.path);
        command
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(dir) = &self.current_dir {
            command.current_dir(dir);
        }
        let mut child = command.spawn()?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        std::thread::scope(|scope| {
            let writer =
                scope.spawn(move || stdin.map_or(Ok(()), |stdin| write_input(stdin, input)));
            let reader = scope.spawn(move || stdout.map_or_else(|| Ok(Vec::new()), read_all));
            let forwarded =
                stderr.map_or_else(|| Ok(Vec::new()), |stderr| forward(stderr, stderr_sink));
            let status = child.wait();
            let read = join(reader);
            let written = join(writer);

            let exit = ScriptExit::of(status?)?;
            written?;
            Ok(ScriptOutput::decode(exit, read?, &forwarded?))
        })
    }
}

/// Writes `input` to the script's stdin, tolerating a script that stops reading early.
fn write_input(mut stdin: ChildStdin, input: &str) -> io::Result<()> {
    match stdin.write_all(input.as_bytes()) {
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        written => written,
    }
}

fn read_all(mut stdout: ChildStdout) -> io::Result<Vec<u8>> {
    let mut captured = Vec::new();
    stdout.read_to_end(&mut captured).map(|_| captured)
}

/// Copies each chunk of stderr to `sink` as it arrives and returns everything read.
fn forward(mut stderr: ChildStderr, sink: &mut impl Write) -> io::Result<Vec<u8>> {
    std::iter::repeat_with(|| read_chunk(&mut stderr))
        .filter(|chunk| !matches!(chunk, Err(error) if error.kind() == io::ErrorKind::Interrupted))
        .take_while(|chunk| !matches!(chunk, Ok(bytes) if bytes.is_empty()))
        .map(|chunk| {
            chunk.and_then(|bytes| {
                sink.write_all(&bytes)
                    .and_then(|()| sink.flush())
                    .map(|()| bytes)
            })
        })
        .collect::<io::Result<Vec<_>>>()
        .map(|chunks| chunks.concat())
}

fn read_chunk(source: &mut impl Read) -> io::Result<Vec<u8>> {
    let mut chunk = [0; 8192];
    source
        .read(&mut chunk)
        .map(|length| chunk[..length].to_vec())
}

/// Joins a stream thread; a panic in it becomes an I/O error.
fn join<T>(handle: ScopedJoinHandle<'_, io::Result<T>>) -> io::Result<T> {
    handle
        .join()
        .unwrap_or_else(|_| Err(io::Error::other("script stream thread panicked")))
}

/// An unconditional state transition morphism backed by an external Unix script.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptStep {
    pub script: Script,
}

impl ScriptStep {
    /// Creates a new [`ScriptStep`] pointing to the given script path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            script: Script::new(path),
        }
    }
}

impl Step<String> for ScriptStep {
    type Out = String;
    type Error = ScriptError;

    fn apply(&self, input: String) -> Result<Self::Out, Self::Error> {
        let output = self.script.run(&input, &mut io::stderr())?;
        match output.exit {
            ScriptExit::Code(0) => Ok(output.stdout),
            exit => Err(ScriptError::Failed {
                exit,
                stderr: output.stderr,
            }),
        }
    }
}

/// A conditional decision coproduct morphism backed by an external Unix script.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScriptBranch {
    pub script: Script,
}

impl ScriptBranch {
    /// Creates a new [`ScriptBranch`] pointing to the given script path.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            script: Script::new(path),
        }
    }
}

impl Branch<String> for ScriptBranch {
    type Left = String;
    type Right = ScriptOutput;
    type Error = ScriptError;

    fn evaluate(&self, input: String) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        let output = self.script.run(&input, &mut io::stderr())?;
        Ok(match output.exit {
            ScriptExit::Code(0) => Either::Left(output.stdout),
            _ => Either::Right(output),
        })
    }
}
