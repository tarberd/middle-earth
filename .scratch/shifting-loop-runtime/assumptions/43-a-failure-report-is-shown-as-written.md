# A failure report is shown as the script wrote it

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`spec.md` §4: "**Failure report.** The report passed to rollback is the failing step's stderr, or its stdout when stderr is empty". §7: "errors exit non-zero with the error on stderr."

## Assumed
An error that carries a failure report shows the report exactly as the script wrote it, trailing newline included. When the report ends in a newline, the error is followed by a blank line on stderr. The runtime doesn't trim the report.

## Proposed spec text
§7 Errors: "An error shows the failure report as the script wrote it, without trimming."
