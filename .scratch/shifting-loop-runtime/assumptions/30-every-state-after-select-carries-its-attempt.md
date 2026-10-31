# Every state after select carries its attempt

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §6: "`Selected{task, base}` · `WorkDone{task, base, summary}` · `Verified{task, base, output}` · …".

`.scratch/shifting-loop-runtime/assumptions/19-iteration-states-carry-the-iteration-number.md`: "Every state from `Selected` to `Declined` also carries its iteration number".

## Assumed
The iteration number, the task and the base commit always travel together, so the states group them as one value, the iteration's attempt. Every state from `Selected` to `Declined` holds the attempt plus its own data: `Selected{attempt}`, `WorkDone{attempt, summary}`, `Verified{attempt, summary}`, `Approved{attempt, verdict}`, `AlreadyDone{attempt, output}`, `Rejected{attempt, outcome, report}` and `Declined{attempt, reason}`. Each state still holds exactly what §6 and assumptions 19–20 give it. Only the grouping changes.

## Proposed spec text
§6: "Every state from `Selected` to `Declined` carries the iteration number, the task and the base commit, plus its own data. They may be separate fields of each state, as listed above, or one attempt value that each state holds."
