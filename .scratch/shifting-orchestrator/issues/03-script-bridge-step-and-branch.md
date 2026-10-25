# Unix Script Bridge Step and Branch Adapters

Status: claimed
Blocked by: 01

## Description

Implement the Unix script execution bridge connecting external scripts to the trait system without temporary disk state:
- Implement `ScriptOutput { pub code: i32, pub stdout: String, pub stderr: String }` representing child process execution outcomes.
- Implement direct process execution via `std::process::Command::new(script_path)` without intermediate `/bin/sh` subshells (requiring executable bits `chmod +x` and standard shebangs).
- Stream input state payloads over child STDIN, capturing STDOUT and STDERR into `ScriptOutput`.
- Implement `ScriptStep`: spawns executable, pipes input string via STDIN, captures STDOUT on exit `0`, returns typed `ScriptError::NonZeroExit` on non-zero exit status.
- Implement `ScriptBranch`: spawns executable, pipes input string via STDIN, maps exit `0` to `Ok(Either::Left(stdout))` and non-zero exit to `Ok(Either::Right(ScriptOutput))` allowing downstream categorical branching on exit code without ad-hoc API extensions.
- Typed process errors via `thiserror` in `ScriptError` (`Io(std::io::Error)`, `NonZeroExit { code: i32, stderr: String }`, `Signaled`).

## Verification Seam

**`Script Bridge` Seam**: Child process spawning, STDIN $\to$ STDOUT/STDERR stream piping, `ScriptOutput`, and `ScriptError`.
- Catches: Process spawning failures, broken pipes, stream encoding defects, unhandled exit status.
- Misses: Iterative loop convergence, CLI argument handling.

## Acceptance Criteria

- [ ] `ScriptOutput { pub code: i32, pub stdout: String, pub stderr: String }` struct with standard trait derivations.
- [ ] Direct binary invocation via `std::process::Command::new(path)` (no subshells).
- [ ] All inter-process communication uses Unix STDIN/STDOUT stream piping without writing temporary disk state.
- [ ] `ScriptStep` implements `Step<String, Out = String, Error = ScriptError>`.
- [ ] `ScriptBranch` implements `Branch<String, Left = String, Right = ScriptOutput, Error = ScriptError>`.
- [ ] Typed `ScriptError` using `thiserror` for `Io`, `NonZeroExit`, and `Signaled`.
- [ ] Integration tests using mock shell scripts verifying exit code 0, non-zero exits, STDIN stream delivery, and missing script error propagation.
- [ ] Passes `cargo test` and `cargo clippy --all-targets -- -D warnings`.
