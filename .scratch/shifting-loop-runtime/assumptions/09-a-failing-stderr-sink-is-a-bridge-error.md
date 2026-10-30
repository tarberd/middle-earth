# A failing stderr sink is a bridge error

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
Ticket 01: "Every way a script can end comes back as a result the caller can route".

`spec.md` §8: "Stderr is copied live to shift's stderr while it is being captured, so no pipe can deadlock."

## Assumed
When copying stderr to the sink fails, for example because shift's stderr is a closed pipe, the Script Bridge returns an I/O error and stops capturing. A broken sink is a failure of shift, not a way the script ended, so it isn't routed as a script outcome. The script may then die of SIGPIPE. Nothing deadlocks.

## Proposed spec text
§7 Errors: "Failing to copy a script's stderr to shift's stderr is an I/O error and stops the run."
