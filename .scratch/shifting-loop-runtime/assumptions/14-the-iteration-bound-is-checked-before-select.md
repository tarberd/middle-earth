# The iteration bound is checked before select

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §7: "`MaxIterations`". Story 5: "an optional `--max-iterations` bound on every command, so that I can cap an experiment the way `./loop.sh 20` does."

## Assumed
Before each iteration, and before its select, the loop checks the bound. A loop that has already run `--max-iterations` iterations ends `MaxIterations` without running select again, even if select would have reported no work left. `--max-iterations 0` runs no script. This matches `./loop.sh 20`, which checks its bound at the top of each iteration.

## Proposed spec text
§7: "`MaxIterations`: the loop has run `--max-iterations` iterations. The bound is checked before select, so a bound of `0` runs no script."
