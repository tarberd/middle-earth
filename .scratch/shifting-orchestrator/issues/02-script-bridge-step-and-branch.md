# Script Bridge Step and Branch Adapters

Status: ready-for-agent
Blocked by: 01

## Description

Implement the Unix script execution bridge connecting external scripts to the trait system:
- Implement `ScriptStep`: spawns an executable command/script, pipes input payload string via STDIN, captures STDOUT as output string, and returns an error if exit status != 0.
- Implement `ScriptBranch`: spawns an executable command/script, pipes input payload string via STDIN. Exit code `0` maps to `Either::Left(stdout)`; non-zero exit code maps to `Either::Right(stderr)`.
- Support exit code `2` on `ScriptStep` signaling loop termination cleanly without treating it as a crash.

## Acceptance Criteria

- [ ] All inter-process communication uses Unix STDIN/STDOUT stream piping without writing temporary disk state.
- [ ] Proper process error propagation: unhandled crashes and missing script paths return typed `std::io::Error`.
- [ ] Integration tests using mock shell scripts verifying exit code 0, non-zero, and exit code 2 semantics.
- [ ] Passes `cargo test` and `cargo clippy --all-targets -- -D warnings`.
