# 01: Script Bridge: environment, working directory, signal exits and live stderr

Status: claimed
Blocked by: None (can start immediately)

## What to build

Prefactor the Script Bridge so the shift loop (ticket 02) can host scripts on the contract in the spec's sections 4 and 8. Today a script runs with shift's own working directory and environment, its stderr is buffered until exit, a kill by signal is a bridge error, and invalid UTF-8 on either stream is a bridge error. After this ticket a caller can run a script with its own environment variables and working directory and watch its stderr live. Every way a script can end comes back as a result the caller can route: a kill by signal or invalid stdout is a failed run, never a bridge error.

`ScriptStep` and `ScriptBranch` keep their stdin/stdout convention. The prototype default loop still uses them and must keep compiling and passing until ticket 02 removes it.

## Acceptance criteria

- [ ] A script invoked through the bridge receives the environment variables passed for that invocation on top of shift's inherited environment.
- [ ] A script invoked through the bridge runs in the working directory passed for that invocation.
- [ ] A script killed by a signal produces a failed exit that names the signal, which `ScriptBranch` delivers on its right arm like any non-zero exit, instead of an error.
- [ ] Stdout that isn't valid UTF-8 counts as the script failing, not as a bridge error. Stderr is decoded lossily.
- [ ] Stderr is forwarded to a live sink as it is written and is also captured in full. A test observes forwarded stderr before the script exits.
- [ ] Large stdout and large stderr written at the same time still complete without deadlock. Broken pipes on stdin are still tolerated.
- [ ] The failure report convention is unchanged: the failing script's stderr, or its stdout when stderr is empty.
- [ ] Script Bridge seam tests cover environment variables, working directory, signal exits, invalid UTF-8 stdout, lossy stderr and live stderr.
- [ ] Tests and Clippy pass with warnings denied across all targets.
