# From conversions may rely on a caller's check

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`CODING_STANDARDS.md` §3: "**Parse, Don't Validate**: Encode invariants directly into algebraic data types (ADTs) at boundaries."

## Assumed
A `From` conversion may turn every value of a type into an error, as long as each caller converts only values it has just checked are failures. For example, a script's output may become a failed-script error right after the caller checks that the run didn't succeed.

## Proposed spec text
`CODING_STANDARDS.md` §3: "A `From` conversion may rely on a check its callers make, when every caller makes that check right before converting."
