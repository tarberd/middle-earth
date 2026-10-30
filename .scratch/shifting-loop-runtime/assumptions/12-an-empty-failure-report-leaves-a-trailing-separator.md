# An empty failure report leaves a trailing separator

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
Nothing.

## Assumed
The error for a failed script whose failure report is empty ends with a trailing separator, e.g. `script failed (signal: 9 (SIGKILL)): `. It is left as is.

## Proposed spec text
§7 Errors: "An error for a failed script names how it ended, then its failure report, which may be empty."
