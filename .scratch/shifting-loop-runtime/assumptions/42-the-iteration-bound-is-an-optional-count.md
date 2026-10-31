# The iteration bound is an optional count

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
Ticket 02: "`--max-iterations` (unbounded when absent)".

`code-review` skill baseline: "**Primitive Obsession**: a primitive or string standing in for a domain concept that deserves its own type."

## Assumed
The iteration bound is "no bound, or this many iterations", which an optional count already says. The rules that use it, ending at the bound and continuing past the last iteration number, belong to the idle state that holds the iteration number. So the bound gets no type of its own.

## Proposed spec text
§7: "The iteration bound is either absent or a count of iterations."
