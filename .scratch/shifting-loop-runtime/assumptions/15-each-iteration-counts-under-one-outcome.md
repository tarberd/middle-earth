# Each iteration counts under one outcome

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §7: "a loop summary: iterations, commits, already-done tasks, rejections counted per outcome, and declines." The Solution says: "An iteration ends committed, already done, rejected (work failed, verify failed or judge failed) or declined."

## Assumed
Each iteration counts under exactly one outcome, and the iteration count is their sum. "Commits" counts only iterations whose judge passed (`verified`). An already-done iteration also runs commit, but it counts as already done, not as a commit. Rejections are counted per outcome as `work-failed`, `verify-failed` and `judge-failed`.

## Proposed spec text
§7: "A loop summary counts each iteration under one outcome: commits (`verified`), already-done tasks, `work-failed`, `verify-failed` and `judge-failed` rejections, and declines. Iterations is their sum."
