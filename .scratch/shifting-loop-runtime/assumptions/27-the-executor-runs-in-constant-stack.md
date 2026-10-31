# The Executor runs in constant stack

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`spec.md` §1: "The category-theory core (Step, Branch, Either, LoopFlow, Executor and their identities, injections and combinators) and the Script Bridge are kept and reused."

`spec.md`, Solution: "It never stops on its own while select still returns work." Ticket 02: "`--max-iterations` (unbounded when absent)".

## Assumed
Keeping the Executor keeps its interface and behaviour, not how it runs. It runs any number of iterations in constant stack, so an unbounded loop never ends in a stack overflow.

## Proposed spec text
§1: "The Executor keeps its interface and runs any number of iterations in constant stack."
