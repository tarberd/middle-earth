# Unix Script Bridge Step and Branch Adapters

Status: resolved
Blocked by: 01

## Description

Implement the Unix script execution bridge connecting external scripts to the trait system without temporary disk state:
- Implement `ScriptOutput { pub code: i32, pub stdout: String, pub stderr: String }` representing child process execution outcomes.
- Implement direct process execution via `std::process::Command::new(script_path)` without intermediate `/bin/sh` subshells (requiring executable bits `chmod +x` and standard shebangs).
- Stream input state payloads over child STDIN, capturing STDOUT and STDERR into `ScriptOutput`.
- Implement `ScriptStep`: spawns executable, pipes input string via STDIN, captures STDOUT on exit `0`, returns typed `ScriptError::NonZeroExit` on non-zero exit status.
- Implement `ScriptBranch`: spawns executable, pipes input string via STDIN, maps exit `0` to `Ok(Either::Left(stdout))` and non-zero exit to `Ok(Either::Right(ScriptOutput))` allowing downstream categorical branching on exit code without ad-hoc API extensions.
- Typed process errors via `thiserror` in `ScriptError` (`Io(std::io::Error)`, `NonZeroExit { code: i32, stderr: String }`, `Signaled`, `Utf8(std::string::FromUtf8Error)`).

## Verification Seam

**`Script Bridge` Seam**: Child process spawning, STDIN $\to$ STDOUT/STDERR stream piping, `ScriptOutput`, and `ScriptError`.
- Catches: Process spawning failures, broken pipes, stream encoding defects, unhandled exit status.
- Misses: Iterative loop convergence, CLI argument handling.

## Acceptance Criteria

- [x] `ScriptOutput { pub code: i32, pub stdout: String, pub stderr: String }` struct with standard trait derivations.
- [x] Direct binary invocation via `std::process::Command::new(path)` (no subshells).
- [x] All inter-process communication uses Unix STDIN/STDOUT stream piping without writing temporary disk state.
- [x] `ScriptStep` implements `Step<String, Out = String, Error = ScriptError>`.
- [x] `ScriptBranch` implements `Branch<String, Left = String, Right = ScriptOutput, Error = ScriptError>`.
- [x] Typed `ScriptError` using `thiserror` for `Io`, `NonZeroExit`, `Signaled`, and `Utf8`.
- [x] Integration tests using mock shell scripts verifying exit code 0, non-zero exits, STDIN stream delivery, missing script error propagation, invalid UTF-8 output, broken pipe early termination, and >64KB deadlock-free pipe streaming.
- [x] Passes `cargo test` and `cargo clippy --all-targets -- -D warnings`.
