# Script step errors carry the failure report

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`spec.md` §4: "**Failure report.** The report passed to rollback is the failing step's stderr, or its stdout when stderr is empty, matching the current Script Bridge behaviour."

Ticket 01: "The failure report convention is unchanged: the failing script's stderr, or its stdout when stderr is empty."

## Assumed
The convention covers every failure the Script Bridge reports, not only rollback's stdin. A failed `ScriptStep`'s error names how the script ended and carries its failure report: stderr, or stdout when stderr is empty. Before this ticket that error carried stderr only.

## Proposed spec text
§4 Failure report: "Every failure the Script Bridge reports, to rollback or as an error, carries the failure report: the failing step's stderr, or its stdout when stderr is empty."
