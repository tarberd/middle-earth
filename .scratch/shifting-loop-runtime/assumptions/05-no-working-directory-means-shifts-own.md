# No working directory means shift's own

Filed by: implementer (shifting-loop-runtime 01)

## Spec says
`spec.md` §5: "Every script runs with the repository root as its working directory and inherits shift's environment".

## Assumed
The Script Bridge doesn't default to the repository root. An invocation may leave its working directory unset, and the script then runs in shift's own working directory. The loops (ticket 02) name the repository root on every invocation.

## Proposed spec text
§8: "When an invocation names no working directory, the script runs in shift's working directory. The loops always name the repository root."
