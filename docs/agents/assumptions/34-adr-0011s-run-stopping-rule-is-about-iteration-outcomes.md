# ADR-0011's run-stopping rule is about iteration outcomes

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`docs/adr/0011-shifting-category-theory-type-state-orchestrator.md`, Amendment, Outcomes: "Only failing commit or rollback scripts stop a run."

`.scratch/shifting-loop-runtime/spec.md` §7: "**Errors**, typed: precondition failures; missing directory or script; select error; commit or rollback failure; review session failure; I/O and Script Bridge errors."

## Assumed
This flags a conflict between ADR-0011 and the loop-runtime spec. The ADR line compares the ways an iteration can end. A work failure or a decline never stops a run, and of the scripts that settle an iteration, only a failing commit or rollback does. It doesn't rule out the errors the spec lists, so those stop the run too, each with a typed error:
- a select error;
- a failed git query;
- a count overflow;
- a Script Bridge error from any loop script.

The ADR's wording needs amending; the behaviour stays.

## Proposed spec text
ADR-0011, Amendment, Outcomes: "Of the scripts that settle an iteration, only a failing commit or rollback stops a run. A run also stops on errors outside the iteration outcomes, such as a select error or a Script Bridge error."
