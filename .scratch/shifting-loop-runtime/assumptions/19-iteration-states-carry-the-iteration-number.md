# Iteration states carry the iteration number

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §6: "`Idle{iteration}` · `Selected{task, base}` · …" and "After a commit, or a rollback of a rejection, the iteration yields `LoopFlow::Continue(Idle)`. After a rollback of a decline it yields `LoopFlow::Break(Declined)`."

## Assumed
- Every state from `Selected` to `Declined` also carries its iteration number, so each script receives `SHIFT_ITERATION`.
- `Idle` also carries the loop summary so far, since the Executor's only state between iterations is `Idle`.
- Commit or rollback yields the iteration's task and outcome. A pure transition then counts the outcome in the summary and yields `Continue(Idle)` with the next iteration, or `Break` with the loop's end state and summary: `Declined{task}` after a decline, `Exhausted` when select reported no work left.

## Proposed spec text
§6: "`Idle{iteration, summary}`; every later state also carries `iteration`. Commit or rollback yields the task and outcome, and a pure transition maps them to `LoopFlow::Continue(Idle)` or `LoopFlow::Break` with the end state and summary."
