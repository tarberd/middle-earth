# The prototype loop may change behaviour

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
Ticket 01: "The prototype default loop still uses them and must keep compiling and passing until ticket 02 removes it."

## Assumed
The prototype default loop only has to compile and pass its tests. Its behaviour may change with the new Script Bridge:
- it echoes every script's stderr live;
- a `verify.sh` killed by a signal, or writing invalid stdout, rolls back instead of aborting the run.

No new tests cover those routes.

## Proposed spec text
Ticket 01: "The prototype default loop still uses them and must keep compiling and passing until ticket 02 removes it. Its behaviour may change with the Script Bridge."
