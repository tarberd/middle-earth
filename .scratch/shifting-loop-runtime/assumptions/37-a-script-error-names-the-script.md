# A script error names the script

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`spec.md` §7: "**Errors**, typed: … I/O and Script Bridge errors." Story 10: "report a missing loop directory or script by its path before running anything, so that configuration mistakes surface before any agent is called."

`.scratch/shifting-loop-runtime/assumptions/25-a-required-script-is-a-file.md`: "A script that can't be run stops the run with a Script Bridge error when the loop reaches it."

## Assumed
Every error raised while running a loop script names that script's path. This covers a script that exists but can't be executed, and select, commit and rollback failures, so the operator can tell which script stopped the run.

## Proposed spec text
§7 Errors: "An error raised while running a loop script names the script's path."
