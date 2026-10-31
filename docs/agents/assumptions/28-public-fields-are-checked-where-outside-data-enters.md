# Public fields are checked where outside data enters

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`CODING_STANDARDS.md` §3: "**Parse, Don't Validate**: Encode invariants directly into algebraic data types (ADTs) at boundaries."

`CODING_STANDARDS.md` §4: "**No Setters and Getters**: Treat all objects as immutable with public attributes".

## Assumed
With public attributes, any code can build a value, so a newtype over a primitive can't stop code from building a value its boundary would reject. An example is a task reference with a line break (`.scratch/shifting-loop-runtime/assumptions/28-a-task-reference-is-one-line.md`). §3 is satisfied when the boundary parse that turns outside data into the value enforces the invariant, and is the only route from outside data to that value.

## Proposed spec text
`CODING_STANDARDS.md` §3: "When public attributes can't carry an invariant on their own, the boundary parse that turns outside data into the value enforces it."
