# Iterations are numbered from one

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §5: "`SHIFT_ITERATION` | iteration number within the run, counted across loops".

## Assumed
A run's first iteration is number `1`. An iteration counts once select picks a task. A select that reports no work left gets the number the next iteration would have had, and doesn't count as an iteration.

## Proposed spec text
§5: "`SHIFT_ITERATION` | iteration number within the run, counted across loops from `1`. A select that reports no work left receives the next iteration's number, and that run of select doesn't count as an iteration."
