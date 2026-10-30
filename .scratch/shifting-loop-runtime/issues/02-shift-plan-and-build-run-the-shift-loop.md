# 02: `shift plan` and `shift build` run the shift loop, replacing the prototype

Status: ready-for-agent
Blocked by: 01

## What to build

An operator runs `shift build` (or `shift plan`) in a git repository whose `.shifting/build/` (or `.shifting/plan/`) holds the seven scripts. Each iteration of the shift loop runs select, precheck, work, verify and judge, then commit or rollback, exactly as the spec's script contract table defines (sections 4–7). The loop ends `Exhausted`, `Declined{task}` or `MaxIterations`, prints its loop summary and then its end state as the last stdout line, and exits `0`.

The prototype five-script default loop is removed in the same change, with no compatibility layer (spec section 1). The category-theory core and the Script Bridge are reused.

`SHIFT_RUN_DIR` and `SHIFT_ITERATION_DIR` arrive with the run directory in ticket 04. Repository preconditions arrive in ticket 03.

## Acceptance criteria

- [ ] `DefaultShiftLoop`, `PlanBranch`, `VerifyResolution`, `VerifyAndResolve`, five-script discovery, the `--dir` and `--verbose` options, the repository-root `.shifting/` stub scripts and the tests that exercise them are gone.
- [ ] `shift plan` and `shift build` run the plan and build loops from `.shifting/plan/` and `.shifting/build/`. The global options are the repository root (default: the current directory, resolved through git) and `--max-iterations` (unbounded when absent).
- [ ] A missing loop directory or required script is reported by its path before any script runs.
- [ ] Every script runs with the repository root as its working directory and receives `SHIFT_LOOP`, `SHIFT_ITERATION`, `SHIFT_TASK` (unset for select), `SHIFT_BASE` (HEAD when the iteration started) and, for commit and rollback, `SHIFT_OUTCOME`.
- [ ] Select: `0` makes its trimmed stdout the task reference; `2` ends the loop `Exhausted`; any other exit or a kill by signal stops the run with an error showing select's stderr.
- [ ] Precheck: `0` skips work, verify and judge and commits as `already-done`, with precheck's stdout on commit's stdin. Any other exit or a kill by signal means the task needs work.
- [ ] Work: `0` passes its stdout as the work summary to verify; `3` is a decline, which rolls back with `declined` and work's stdout on rollback's stdin, then ends the loop `Declined{task}`. Any other exit or a kill by signal rolls back with `work-failed`, and the loop continues.
- [ ] Verify runs only after work succeeds, and judge only after verify passes. Each receives the work summary. A failure or kill rolls back with `verify-failed` or `judge-failed`, and the loop continues. When judge passes, commit runs with `verified` and judge's stdout on its stdin.
- [ ] Rollback's stdin carries the failing step's report (its stderr, or its stdout when stderr is empty).
- [ ] A non-zero exit or a kill of commit or rollback stops the run with a typed error and a non-zero exit status.
- [ ] The loop is composed from Step, Branch and Executor over the typed states in spec section 6, so illegal transitions don't compile.
- [ ] Pure functions map exit status to outcome and accumulate the loop summary (iterations, commits, already-done tasks, rejections per outcome, declines). Tests call them with in-memory values.
- [ ] Errors exit non-zero with the error on stderr.
- [ ] `shift` CLI seam tests against a temporary git repository with logging stub scripts cover spec scenarios 1–5 and 7 (including `--max-iterations 0`), and parsing of `plan`, `build` and the global options.
- [ ] Tests and Clippy pass with warnings denied across all targets.
