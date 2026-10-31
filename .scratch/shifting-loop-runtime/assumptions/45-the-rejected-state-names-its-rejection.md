# The rejected state names its rejection

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`spec.md` §6: "`Rejected{task, base, outcome, report}`".

`.scratch/shifting-loop-runtime/assumptions/30-every-state-after-select-carries-its-attempt.md`: "Each state still holds exactly what §6 and assumptions 19–20 give it. Only the grouping changes."

## Assumed
The rejected state holds what §6 calls its outcome, meaning which failure rejected the iteration: work, verify or judge. It holds it under the name `rejection`. That keeps it apart from the iteration outcome that commit and rollback receive, which also covers commits, already-done tasks and declines.

## Proposed spec text
§6: "`Rejected{task, base, rejection, report}`, where `rejection` is work failed, verify failed or judge failed."
