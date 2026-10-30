# The Script Bridge Rust API may break

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
Ticket 01: "`ScriptStep` and `ScriptBranch` keep their stdin/stdout convention."

`spec.md` §1, about the prototype loop: "No compatibility shims, deprecation paths or migration of the old convention."

## Assumed
Only the stdin/stdout convention must survive. The Script Bridge's Rust types, error variants and fields may change without compatibility. This ticket replaced the separate non-zero-exit, signal and UTF-8 errors with one failed-script error, and changed the shape of `ScriptStep`, `ScriptBranch` and the captured script output.

## Proposed spec text
Ticket 01: "The Script Bridge's Rust API may change without compatibility. Only the stdin/stdout convention is kept."
