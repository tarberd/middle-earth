use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::traits::{ProcessError, ProcessHandle, ProcessOutput, ProcessRunner};

/// Records an individual process operation executed on the mock runner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordedProcessAction {
    Execute {
        program: String,
        arguments: Vec<String>,
    },
    SpawnDaemon {
        program: String,
        arguments: Vec<String>,
    },
    KillDaemon {
        process_identifier: u32,
    },
}

#[derive(Debug, Default)]
struct MockProcessState {
    recorded_actions: Vec<RecordedProcessAction>,
    simulated_outputs: HashMap<String, ProcessOutput>,
    injected_errors: HashMap<String, String>,
    killed_processes: Vec<u32>,
    next_process_identifier: u32,
}

/// In-memory mock handle tracking daemon lifecycle.
struct MockProcessHandle {
    state: Arc<Mutex<MockProcessState>>,
    process_identifier: u32,
}

impl ProcessHandle for MockProcessHandle {
    fn process_id(&self) -> u32 {
        self.process_identifier
    }

    fn kill(&mut self) -> Result<(), ProcessError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| ProcessError::IoError {
                program: format!("pid-{}", self.process_identifier),
                source: std::io::Error::other(poison_error.to_string()),
            })?;

        locked_state.killed_processes.push(self.process_identifier);
        locked_state
            .recorded_actions
            .push(RecordedProcessAction::KillDaemon {
                process_identifier: self.process_identifier,
            });

        Ok(())
    }
}

/// In-memory mock implementing the ProcessRunner trait for hermetic testing.
#[derive(Debug, Default, Clone)]
pub struct MockProcessRunner {
    state: Arc<Mutex<MockProcessState>>,
}

impl MockProcessRunner {
    /// Creates an empty MockProcessRunner.
    pub fn new() -> Self {
        let state = MockProcessState {
            next_process_identifier: 1000,
            ..Default::default()
        };
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }

    /// Functional builder: registers a simulated output for a program name.
    pub fn with_output(
        self,
        program: impl Into<String>,
        exit_code: i32,
        stdout: impl Into<String>,
        stderr: impl Into<String>,
    ) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state.simulated_outputs.insert(
                program.into(),
                ProcessOutput {
                    exit_code,
                    stdout: stdout.into(),
                    stderr: stderr.into(),
                },
            );
        }
        self
    }

    /// Functional builder: injects an error for a program name.
    pub fn with_injected_error(
        self,
        program: impl Into<String>,
        stderr_details: impl Into<String>,
    ) -> Self {
        if let Ok(mut locked_state) = self.state.lock() {
            locked_state
                .injected_errors
                .insert(program.into(), stderr_details.into());
        }
        self
    }

    /// Returns a copy of all recorded actions executed on this mock.
    pub fn recorded_actions(&self) -> Vec<RecordedProcessAction> {
        self.state
            .lock()
            .map(|locked_state| locked_state.recorded_actions.clone())
            .unwrap_or_default()
    }

    /// Returns all process identifiers that received a kill command.
    pub fn killed_processes(&self) -> Vec<u32> {
        self.state
            .lock()
            .map(|locked_state| locked_state.killed_processes.clone())
            .unwrap_or_default()
    }
}

impl ProcessRunner for MockProcessRunner {
    fn execute(&self, program: &str, arguments: &[&str]) -> Result<ProcessOutput, ProcessError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| ProcessError::IoError {
                program: program.to_string(),
                source: std::io::Error::other(poison_error.to_string()),
            })?;

        locked_state
            .recorded_actions
            .push(RecordedProcessAction::Execute {
                program: program.to_string(),
                arguments: arguments
                    .iter()
                    .map(|argument| (*argument).to_string())
                    .collect(),
            });

        if let Some(error_details) = locked_state.injected_errors.get(program) {
            Err(ProcessError::CommandExecutionFailed {
                program: program.to_string(),
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else {
            let output = locked_state
                .simulated_outputs
                .get(program)
                .cloned()
                .unwrap_or_else(|| ProcessOutput {
                    exit_code: 0,
                    stdout: String::new(),
                    stderr: String::new(),
                });

            Ok(output)
        }
    }

    fn spawn_daemon(
        &self,
        program: &str,
        arguments: &[&str],
    ) -> Result<Box<dyn ProcessHandle>, ProcessError> {
        let mut locked_state = self
            .state
            .lock()
            .map_err(|poison_error| ProcessError::IoError {
                program: program.to_string(),
                source: std::io::Error::other(poison_error.to_string()),
            })?;

        locked_state
            .recorded_actions
            .push(RecordedProcessAction::SpawnDaemon {
                program: program.to_string(),
                arguments: arguments
                    .iter()
                    .map(|argument| (*argument).to_string())
                    .collect(),
            });

        if let Some(error_details) = locked_state.injected_errors.get(program) {
            Err(ProcessError::CommandExecutionFailed {
                program: program.to_string(),
                exit_code: Some(1),
                stderr: error_details.clone(),
            })
        } else {
            let process_identifier = locked_state.next_process_identifier;
            locked_state.next_process_identifier += 1;

            Ok(Box::new(MockProcessHandle {
                state: Arc::clone(&self.state),
                process_identifier,
            }))
        }
    }
}
