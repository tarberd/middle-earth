# 04: Run directory, step logs and progress lines

Status: ready-for-agent
Blocked by: 02

## What to build

An operator can watch a loop as it runs and inspect any iteration afterwards (spec sections 8 and 9). Every script's stderr streams to the terminal as it is written, through the live stderr from ticket 01, and shift prints a progress line on stderr for each step and outcome.

Each run gets its own timestamped run directory under `shifting/runs/` inside the repository's git directory, as git reports it, which also handles worktrees. The run directory holds one numbered directory per iteration and the run summary. Because it is inside the git directory, it never shows up in `git status`, is never committed, and survives rollback's clean. The runtime never deletes run directories.

## Acceptance criteria

- [ ] Each run creates a new timestamped run directory under `shifting/runs/` inside the repository's git directory.
- [ ] Each iteration gets a numbered directory recording its loop, task, base commit and outcome, plus each step's stdout and stderr, written when the step finishes.
- [ ] The run directory holds the run summary when the run ends.
- [ ] Every script receives `SHIFT_RUN_DIR` and `SHIFT_ITERATION_DIR`.
- [ ] Stderr written by a long-running stub appears on shift's stderr before the stub exits (spec scenario 8), and is also saved in the iteration directory along with stdout.
- [ ] shift prints a progress line on stderr for each step and each outcome. The last stdout line is still the end state.
- [ ] After a run, `git status` is clean and `git clean -fd` leaves the run directory in place (spec scenario 9).
- [ ] Tests and Clippy pass with warnings denied across all targets.
