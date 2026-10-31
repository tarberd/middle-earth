# Commit or rollback is chosen by the state's type

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`spec.md` §6: "`Approved` and `AlreadyDone` go to commit; `Rejected` and `Declined` go to rollback."

Ticket 02: "The loop is composed from Step, Branch and Executor over the typed states in spec section 6, so illegal transitions don't compile."

## Assumed
Each state that ends the decision has its own commit or rollback step, chosen by its type, and the `SHIFT_OUTCOME` that step passes follows from the same state. Choosing the script from the outcome value instead would swap a route the compiler checks for a runtime switch. So the type picks the script, and the state also carries the outcome.

## Proposed spec text
§6: "Commit or rollback is chosen by the state's type; the `SHIFT_OUTCOME` it passes follows from the same state."
