# 05: Review stage and `shift review`

Status: ready-for-agent
Blocked by: 04

## What to build

An operator runs `shift review` to deal with pending items whenever they choose (spec sections 3, 4, 8 and 10). shift checks that `.shifting/review/` holds `select.sh`, `review.sh` and `commit.sh`, then runs review select.

- When items are pending and a terminal is attached, shift hands the operator's terminal to `review.sh` through a new interactive adapter, so the operator can hold a session with an agent. A successful session runs review's `commit.sh`. A failing session stops the run and leaves the operator's edits in place, uncommitted.
- When no terminal is attached, shift prints the pending items instead and doesn't run `review.sh`.

The interactive adapter sits alongside `ScriptStep` and `ScriptBranch`, so other compositions can hand the terminal to a script too.

The spec doesn't name review's end states or its environment, so this ticket settles them (decided while splitting the spec into tickets):

- **End states:** `Reviewed` (review's `commit.sh` ran), `NothingPending` (select exited `2`), `Unattended` (pending items printed, `review.sh` not run).
- **Environment:** `SHIFT_LOOP=review`, `SHIFT_ITERATION` at the run's current count, `SHIFT_BASE` set to HEAD when the review starts, and `SHIFT_RUN_DIR`. `SHIFT_TASK` and `SHIFT_ITERATION_DIR` are unset, because a review isn't an iteration.

## Acceptance criteria

- [ ] `shift review` reports a missing `.shifting/review/` directory or required script by its path before running anything.
- [ ] Review select: `0` means items are pending and its stdout lists them; `2` ends with `NothingPending`; any other exit or a kill by signal stops the run with an error.
- [ ] The interactive adapter runs a script with stdin, stdout and stderr inherited and the same environment as other scripts, captures nothing, and yields its exit status as a Branch. Script Bridge seam tests cover it.
- [ ] A global interactive override takes `auto | always | never`. `auto` detects whether stdin is a terminal.
- [ ] Interactive path: `review.sh` exiting `0` runs review's `commit.sh` and ends `Reviewed`. A non-zero `commit.sh` stops the run with an error.
- [ ] `review.sh` exiting non-zero, or killed by a signal, stops the run with a review session failure error. Review's `commit.sh` doesn't run, and the working tree is left as it is (spec scenario 12).
- [ ] Unattended path (override `never`, or `auto` without a terminal): the pending items are printed, `review.sh` doesn't run, and the review ends `Unattended` (spec scenario 11).
- [ ] Review scripts run from the repository root with the environment above.
- [ ] The end state is the last stdout line. Clean end states exit `0`.
- [ ] CLI parsing tests cover `review` and the interactive override.
- [ ] Tests and Clippy pass with warnings denied across all targets.
