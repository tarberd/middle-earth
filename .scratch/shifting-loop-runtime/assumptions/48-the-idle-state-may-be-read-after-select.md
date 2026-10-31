# The idle state may be read after select

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`spec.md` §6: "The state types are immutable values consumed by each transition (ADR-0010)."

`.scratch/shifting-loop-runtime/assumptions/19-iteration-states-carry-the-iteration-number.md`: "`Idle` also carries the loop summary so far, since the Executor's only state between iterations is `Idle`."

## Assumed
The idle state is a small immutable value that can be copied. Select takes a copy. Once the iteration settles, the same idle value counts the outcome and yields the next state. No transition changes a state in place, which is the property ADR-0010 protects, so "consumed" holds in that sense. The types don't tie the settled outcome to the idle state it came from.

## Proposed spec text
§6: "The state types are immutable values that each transition takes by value. A state that can be copied may be read again by a later transition in the same iteration."
