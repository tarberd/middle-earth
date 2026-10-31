# Variables scoped to some scripts are unset for the rest

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §5: "Every script runs with the repository root as its working directory and inherits shift's environment, plus:" and "`SHIFT_TASK` | select's trimmed stdout (unset for select itself)". `SHIFT_OUTCOME` is given only for commit and rollback.

## Assumed
"Unset" holds even when shift's own environment has the variable, for example when shift runs inside another loop's script. Select never sees a `SHIFT_TASK`. Only commit and rollback see a `SHIFT_OUTCOME`. Every other variable in shift's environment is inherited as is, and the table's variables override any inherited value.

## Proposed spec text
§5: "A variable the table doesn't give a script is unset for it, even when shift's own environment has it."
