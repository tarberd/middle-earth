# A count that can't grow stops the run

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §7: "Each comes with a loop summary: iterations, commits, already-done tasks, rejections counted per outcome, and declines."

`docs/agents/assumptions/23-arithmetic-overflow-is-a-failure-path.md`: "overflow becomes a typed error".

## Assumed
Each count and the iteration number go up to 2^64 − 1. When an iteration ends with an outcome whose count is already there, or the loop would continue past iteration number 2^64 − 1, the run stops with an error naming the count or the iteration number. Nothing wraps or panics. The iterations line of the summary is always the exact sum of the six counts, even past 2^64 − 1.

## Proposed spec text
§7: "A count or iteration number that can't grow stops the run with an error naming it. The summary's iteration total is always exact."
