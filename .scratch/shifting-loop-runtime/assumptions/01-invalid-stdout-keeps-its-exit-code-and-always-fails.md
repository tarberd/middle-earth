# Invalid stdout keeps its exit code and always fails

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`spec.md` §4: "**Encoding.** Stdout must be valid UTF-8; invalid stdout counts as that step failing."

## Assumed
A script whose stdout isn't valid UTF-8 has failed, whatever its exit status. Only exit `0` with valid stdout succeeds. An exit `2` or `3` with invalid stdout is a plain failure, never "no work left" or "declined". The failure still reports the exit status the script returned.

## Proposed spec text
§4 Encoding: "Stdout must be valid UTF-8. Invalid stdout counts as that step failing whatever its exit status, so an exit `2` or `3` with invalid stdout is a plain failure. The failure names the exit status."
