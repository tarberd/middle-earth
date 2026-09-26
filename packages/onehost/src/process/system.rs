use std::io::ErrorKind;
use std::process::{Child, Command, Stdio};

use super::traits::{ProcessError, ProcessHandle, ProcessOutput, ProcessRunner};

/// Handle wrapping an active operating system child process.
pub struct SystemProcessHandle {
    child_process: Option<Child>,
    process_identifier: u32,
}

impl Drop for SystemProcessHandle {
    fn drop(&mut self) {
        if let Some(mut active_child) = self.child_process.take() {
            if let Err(error) = active_child.kill() {
                tracing::debug!(
                    process_id = self.process_identifier,
                    error = %error,
                    "Process already exited or failed to kill in drop"
                );
            }
            if let Err(error) = active_child.wait() {
                tracing::debug!(
                    process_id = self.process_identifier,
                    error = %error,
                    "Failed to wait for process in drop"
                );
            }
        }
    }
}

impl ProcessHandle for SystemProcessHandle {
    fn process_id(&self) -> u32 {
        self.process_identifier
    }

    fn kill(&mut self) -> Result<(), ProcessError> {
        if let Some(mut active_child) = self.child_process.take() {
            active_child.kill().map_err(|io_error| ProcessError::IoError {
                program: format!("pid-{}", self.process_identifier),
                source: io_error,
            })?;
            active_child.wait().map_err(|io_error| ProcessError::IoError {
                program: format!("pid-{}", self.process_identifier),
                source: io_error,
            })?;
        }
        Ok(())
    }
}

/// Production process runner executing real host operating system binaries.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemProcessRunner;

impl SystemProcessRunner {
    /// Creates a new SystemProcessRunner.
    pub fn new() -> Self {
        Self
    }
}

impl ProcessRunner for SystemProcessRunner {
    fn execute(&self, program: &str, arguments: &[&str]) -> Result<ProcessOutput, ProcessError> {
        let command_output = Command::new(program)
            .args(arguments)
            .output()
            .map_err(|io_error| {
                if io_error.kind() == ErrorKind::NotFound {
                    ProcessError::ProgramNotFound {
                        program: program.to_string(),
                        source: io_error,
                    }
                } else {
                    ProcessError::IoError {
                        program: program.to_string(),
                        source: io_error,
                    }
                }
            })?;

        let exit_code = command_output.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&command_output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&command_output.stderr).to_string();

        Ok(ProcessOutput {
            exit_code,
            stdout,
            stderr,
        })
    }

    fn spawn_daemon(
        &self,
        program: &str,
        arguments: &[&str],
    ) -> Result<Box<dyn ProcessHandle>, ProcessError> {
        let spawned_child = Command::new(program)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|io_error| {
                if io_error.kind() == ErrorKind::NotFound {
                    ProcessError::ProgramNotFound {
                        program: program.to_string(),
                        source: io_error,
                    }
                } else {
                    ProcessError::IoError {
                        program: program.to_string(),
                        source: io_error,
                    }
                }
            })?;

        let process_identifier = spawned_child.id();

        Ok(Box::new(SystemProcessHandle {
            child_process: Some(spawned_child),
            process_identifier,
        }))
    }
}
