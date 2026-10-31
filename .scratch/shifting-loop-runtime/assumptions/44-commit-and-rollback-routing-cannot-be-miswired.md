# Commit and rollback routing can't be miswired

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`.scratch/shifting-loop-runtime/assumptions/35-commit-or-rollback-is-chosen-by-the-states-type.md`, proposed text: "Commit or rollback is chosen by the state's type; the `SHIFT_OUTCOME` it passes follows from the same state."

Ticket 02: "The loop is composed from Step, Branch and Executor over the typed states in spec section 6, so illegal transitions don't compile."

## Assumed
Assumption 35's reading stands, but its proposed text names one design. The property it protects: for each state that ends the decision, the compiler checks three things together, so none can be miswired. They are the route to commit or rollback, the `SHIFT_OUTCOME` passed, and the error raised when that script fails. Designs that satisfy it include:
- one step per state type, with each of commit and rollback paired once with its failure error;
- an exhaustive match over a closed set of settlements.

## Proposed spec text
§6: "For each state that ends the decision, the compiler checks its route to commit or rollback, the `SHIFT_OUTCOME` it passes and the error its failure raises. One step per state type, or an exhaustive match over the settlements, both qualify."
