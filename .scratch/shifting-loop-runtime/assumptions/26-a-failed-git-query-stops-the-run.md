# A failed git query stops the run

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §7 Errors: "precondition failures; missing directory or script; select error; commit or rollback failure; review session failure; I/O and Script Bridge errors." It doesn't list a git query failing after the preconditions passed.

## Assumed
Resolving the repository root and reading HEAD at the start of each iteration are git queries. If one fails, for example because a script left HEAD unreadable, the run stops with an error naming the git command and git's stderr. Ticket 03 adds the typed precondition errors in front of these queries.

## Proposed spec text
§7 Errors: "A git query that fails stops the run with an error naming the command and git's stderr."
