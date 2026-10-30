# Ralph claims and resolves in their own commits

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`ralph` skill, step 1: "5. Set `Status: claimed` (or `Status: in-progress`) in the ticket file. 6. Pin the starting base commit: `BASE_SHA=$(git rev-parse HEAD)`." It says nothing about committing either status change.

## Assumed
Claiming and resolving each get their own commit, `docs(<scope>): claim …` and `docs(<scope>): resolve …`, as earlier tickets did. `BASE_SHA` is pinned after the claim commit, so the reviewed diff holds only the implementation.

## Proposed spec text
`ralph` skill, step 1.5: "Set `Status: claimed` and commit it on its own, then pin `BASE_SHA`." Step 3: "Commit the resolved ticket on its own."
