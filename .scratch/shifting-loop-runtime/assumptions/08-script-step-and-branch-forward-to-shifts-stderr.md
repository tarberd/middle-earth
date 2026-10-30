# ScriptStep and ScriptBranch forward to shift's stderr

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`spec.md` §8: "Stderr is copied live to shift's stderr while it is being captured, so no pipe can deadlock."

## Assumed
`ScriptStep` and `ScriptBranch` always copy stderr to shift's own stderr. A caller that needs another destination runs the script through the Script Bridge with its own sink.

## Proposed spec text
§8: "`ScriptStep` and `ScriptBranch` copy stderr to shift's stderr. A caller that needs another destination runs the script with its own sink."
