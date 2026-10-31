# Subcommands and loop kinds are separate

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`spec.md` §11: "Subcommands: none (shift run), `plan`, `build`, `review`."

`spec.md` §3: "`.shifting/plan/` … `.shifting/build/` … `.shifting/review/`".

## Assumed
A subcommand says what shift runs, and a loop kind names a shift loop: its `SHIFT_LOOP` value and its directory. The two sets differ: the shift run is a subcommand but not a loop. So they are separate values, and the command line maps one to the other, even while both name only plan and build.

## Proposed spec text
§11: "A subcommand says what runs. The plan and build loops are named separately, for `SHIFT_LOOP` and their directories."
