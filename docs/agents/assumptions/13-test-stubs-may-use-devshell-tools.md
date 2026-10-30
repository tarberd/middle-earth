# Test stubs may use devShell tools

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`docs/agents/validation.md`: "Invoke all Rust commands through: `nix develop .#onehost -c <command>`".

## Assumed
Tests always run in the devShell, so stub scripts in tests may use tools and options beyond POSIX that the devShell provides. An example is `sleep 0.05`: fractional seconds come from GNU coreutils.

## Proposed spec text
`docs/agents/validation.md`: "Stub scripts in tests may rely on the tools the devShell provides, GNU coreutils included."
