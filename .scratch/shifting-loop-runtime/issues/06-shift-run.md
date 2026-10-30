# 06: Shift run

Status: ready-for-agent
Blocked by: 04, 05

## What to build

An operator runs `shift` with no subcommand and gets a whole shift run: review, planning and building in the right order without sequencing them by hand (spec section 10). shift checks all three loop directories before running anything, then:

```
review   (when review select reports pending items)
loop:
  plan loop   → Exhausted: continue · Declined: end run Declined · MaxIterations: end run
  build loop  → Exhausted: continue · Declined: back to plan loop · MaxIterations: end run
  review      → commit moved HEAD: back to plan loop · otherwise: end run Done
```

A review that ends `Unattended` or `NothingPending` counts as committing nothing. The run ends `Done` only when the build loop ended `Exhausted` and the last review committed nothing. One `--max-iterations` bound caps plan and build iterations together across the whole run. The whole run shares one run directory, with `SHIFT_ITERATION` and the iteration directories numbered across loops.

## Acceptance criteria

- [ ] `shift` with no subcommand reports the first missing directory or script among `.shifting/plan/`, `.shifting/build/` and `.shifting/review/` before running anything.
- [ ] The run starts with review only when review select reports pending items.
- [ ] The plan loop runs until it is exhausted before the build loop starts.
- [ ] A build loop ending `Declined` sends the run back to the plan loop, then building resumes.
- [ ] A plan loop ending `Declined` ends the run `Declined{task}`.
- [ ] A final review whose commit moved HEAD triggers another plan and build round. Otherwise the run ends `Done`.
- [ ] `--max-iterations` counts plan and build iterations together and ends the run `MaxIterations` with exact counts.
- [ ] `SHIFT_ITERATION` and the iteration directories are numbered across loops within one run directory.
- [ ] A pure function maps each loop or review end state to the next stage. Tests call it with in-memory values and cover every end state.
- [ ] The run summary totals each loop, is written to the run directory, and is printed. The run's end state is the last stdout line, and clean end states exit `0`.
- [ ] `shift` CLI seam tests cover spec scenario 10, using the interactive override `always` and a stub `review.sh` for the final-review commit.
- [ ] Tests and Clippy pass with warnings denied across all targets.
