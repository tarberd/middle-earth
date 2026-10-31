# The shared failed-script type comes with ticket 07

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
Ticket 07: "A successful script output can't become the script-failed error: the conversion either accepts only failed outputs or gives no error for a successful one."

`code-review` skill baseline: "**Data Clumps**: the same few fields or params keep travelling together (a type wanting to be born)."

## Assumed
The Script Bridge's failed-script error, select's failure and a commit or rollback failure each carry how the script ended and its failure report. One type for a failed script run would serve all three. That type is the one ticket 07 asks for, the conversion's narrowed input, so ticket 07 introduces it and this ticket leaves the three shapes as they are.

## Proposed spec text
Ticket 07: "The failed script run the conversion accepts also serves the loop's select, commit and rollback failures, which carry how the script ended and its failure report."
