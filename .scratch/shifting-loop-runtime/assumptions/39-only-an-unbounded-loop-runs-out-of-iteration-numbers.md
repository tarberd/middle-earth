# Only an unbounded loop runs out of iteration numbers

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`.scratch/shifting-loop-runtime/assumptions/34-a-count-that-cannot-grow-stops-the-run.md`: "When an iteration ends with an outcome whose count is already there, or the loop would continue past iteration number 2^64 − 1, the run stops with an error naming the count or the iteration number."

`.scratch/shifting-loop-runtime/assumptions/14-the-iteration-bound-is-checked-before-select.md`: "A loop that has already run `--max-iterations` iterations ends `MaxIterations` without running select again".

## Assumed
No bound is higher than 2^64 − 1, so a loop with `--max-iterations` reaches its bound by iteration 2^64 − 1 at the latest, and ends `MaxIterations`. It never stops with the iteration-number error. With `--max-iterations 18446744073709551615`, the loop ends `MaxIterations` once that iteration settles. Only an unbounded loop that settles iteration 2^64 − 1 and would go on stops with that error.

The iteration that just settled is counted first. So when its outcome's count is also full, the error names that count.

## Proposed spec text
§7: "A bounded loop always ends at its bound. Only an unbounded loop can run out of iteration numbers, which stops the run with an error naming the iteration number."
