# Loop errors carry the failure report

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
Ticket 02: "any other exit or a kill by signal stops the run with an error showing select's stderr." `.scratch/shifting-loop-runtime/assumptions/03-script-step-errors-carry-the-failure-report.md`: "Every failure the Script Bridge reports, to rollback or as an error, carries the failure report".

## Assumed
The select error shows how select ended and its failure report: its stderr, or its stdout when stderr is empty. Commit and rollback errors name the task, how the script ended and its failure report.

## Proposed spec text
§7 Errors: "A select, commit or rollback error names how the script ended and its failure report; commit and rollback errors also name the task."
