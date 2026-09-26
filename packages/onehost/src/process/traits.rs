use std::fmt::Debug;
use thiserror::Error;

/// Structured result of a completed process execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Enumerates errors that can occur during external process execution.
#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("Program '{program}' was not found: {source}")]
    ProgramNotFound {
        program: String,
        source: std::io::Error,
    },

    #[error("Process '{program}' failed with exit code {exit_code:?}: {stderr}")]
    CommandExecutionFailed {
        program: String,
        exit_code: Option<i32>,
        stderr: String,
    },

    #[error("I/O error while executing process '{program}': {source}")]
    IoError {
        program: String,
        source: std::io::Error,
    },
}

/// Abstract handle to a spawned background daemon process.
pub trait ProcessHandle: Send + Sync {
    /// Returns the system process ID.
    fn process_id(&self) -> u32;

    /// Terminates the running daemon process.
    fn kill(&mut self) -> Result<(), ProcessError>;
}

/// Trait abstracting external process execution and daemon lifecycle management.
pub trait ProcessRunner: Send + Sync {
    /// Executes a program synchronously, waiting for completion and returning captured output.
    fn execute(&self, program: &str, arguments: &[&str]) -> Result<ProcessOutput, ProcessError>;

    /// Spawns a long-running background daemon process (such as `swtpm`), returning a controllable handle.
    fn spawn_daemon(
        &self,
        program: &str,
        arguments: &[&str],
    ) -> Result<Box<dyn ProcessHandle>, ProcessError>;
}
