# Ralph resolves at the round limit when only smells remain

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`ralph` skill, step 2: "Execute an iterative loop with a maximum safeguard limit of 5 rounds."

`ralph` skill, step C: "**If both axes report clean** (zero hard violations, zero baseline smells, all criteria satisfied, all tests passing): Pass the gate and proceed to Step 3."

## Assumed
The skill doesn't say what happens when the last round's gate isn't clean. Suppose the round limit is reached, every acceptance criterion holds, tests and lints pass, and every hard finding is fixed or settled by a recorded ruling. Then the orchestrator resolves the ticket. Any judgement-call smells it agrees with but has no round left to fix go in its final report, for the human to schedule. If a hard violation or a spec gap remained, the ticket would stay claimed and the orchestrator would report what remains.

## Proposed spec text
`ralph` skill, step 2: "At the round limit, resolve the ticket only when no hard violation or spec gap remains, and report any smells left over. Otherwise leave the ticket claimed and report what remains."
