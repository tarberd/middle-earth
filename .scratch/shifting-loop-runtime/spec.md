# Shifting Loop Runtime: Harness-Owned Plan and Build Loops

Status: ready-for-agent

## Problem Statement

An operator who wants to run a Ralph loop with `shift` today gets a prototype five-script loop that can't host one safely:

- The only thing a script learns about the iteration is a string on stdin, and each step overwrites it. No script knows which task was selected, which commit the iteration started from, or which iteration it is in.
- A failing `work.sh` aborts the whole run instead of rolling back, so one bad agent run ends an unattended loop.
- An agent has no way to say that the task it was given is wrong.
- There is one `verify.sh` and nowhere to put an LLM judge after the deterministic checks.
- Every script's output is buffered until it exits, so a twenty-minute agent run is invisible, and nothing is kept per iteration for later inspection.
- A run ends with "completed N iterations" whether the plan ran out or an iteration bound was hit.
- There is no plan loop, no review stage, and nothing composes them.
- It starts on a dirty working tree, so a rollback can destroy the operator's uncommitted work.
- All stdio is captured, so a script can't hold an interactive session with the operator.

## Solution

The `shift` binary becomes the harness for shifting's only loop: Clayton Farr's Ralph loop, modified so that the harness, not the agent, owns task selection, verification, commit and rollback (ADR-0011 as amended).

- **One loop shape.** Plan and build loops are the same composition of Steps and Branches. Each iteration runs select, precheck, work, verify and judge, then commit or rollback. The plan loop's scripts live in `.shifting/plan/` and the build loop's in `.shifting/build/`.
- **Script context.** Every script receives its context through environment variables and keeps the Script Bridge stdin/stdout convention.
- **Iteration outcomes.** An iteration ends committed, already done, rejected (work failed, verify failed or judge failed) or declined.
  - A work failure rolls back and the loop continues.
  - A decline rolls back and ends the loop.
  - Only a failing commit or rollback stops the run.
- **End states.** A loop ends `Exhausted`, `Declined{task}` or `MaxIterations`, with a summary. It never stops on its own while select still returns work.
- **Logs.** Every script's stderr streams to the terminal live. Each run gets a directory inside the repository's git directory holding every step's output.
- **Review stage.** The review stage hands the operator's terminal to an interactive script.
- **Shift run.** A shift run composes review, the plan loop, the build loop and a final review. It returns to planning after a decline and repeats when review changed the specs.
- **Commands.** `shift`, `shift plan`, `shift build` and `shift review`, each with an optional `--max-iterations`.
- **Preconditions.** shift refuses to run outside a git repository, in one without commits, or on a dirty working tree.
- **Removal.** The prototype default loop and its five-script convention are removed with no compatibility layer.

What the shipped scripts do (task files, prompts, helper subcommands, `shift init`) is specified in `shifting-default-loop`. This spec defines the runtime they plug into and tests it with stub scripts.

## User Stories

1. As an operator, I want `shift plan` to run only the plan loop from `.shifting/plan/`, so that I can plan without building, like Farr's `./loop.sh plan`.
2. As an operator, I want `shift build` to run only the build loop from `.shifting/build/`, so that I can build from an existing plan, like Farr's `./loop.sh`.
3. As an operator, I want `shift` with no subcommand to perform a whole shift run, so that review, planning and building happen in the right order without my sequencing them.
4. As an operator, I want `shift review` to run only the review stage, so that I can review pending items whenever I choose.
5. As an operator, I want an optional `--max-iterations` bound on every command, so that I can cap an experiment the way `./loop.sh 20` does.
6. As an operator, I want loops to run unbounded when I don't pass `--max-iterations`, so that a loop never stops while work remains unless I stop it.
7. As an operator, I want a global option that selects the repository root, defaulting to the current directory, so that I can drive a repository from elsewhere and tests can target temporary repositories.
8. As an operator, I want shift to refuse to start when the working tree has uncommitted or untracked changes, so that a rollback can never destroy my own work.
9. As an operator, I want shift to refuse to start outside a git repository or in a repository without commits, so that every iteration has a base commit to return to.
10. As an operator, I want shift to report a missing loop directory or script by its path before running anything, so that configuration mistakes surface before any agent is called.
11. As a script author, I want every iteration to run select, precheck, work, verify and judge and then commit or rollback, so that the plan and build loops share one shape.
12. As a script author, I want every script to run with the repository root as its working directory, so that scripts can use relative paths.
13. As a script author, I want every script to receive `SHIFT_LOOP`, `SHIFT_ITERATION`, `SHIFT_TASK`, `SHIFT_BASE`, `SHIFT_RUN_DIR` and `SHIFT_ITERATION_DIR`, so that scripts know what they are working on without parsing stdin.
14. As a script author, I want select's trimmed stdout to become `SHIFT_TASK` and to be treated as an opaque reference, so that the runtime stays independent of how tasks are stored.
15. As a script author, I want select exiting `2` to end the loop as `Exhausted`, so that a loop ends only when select says no work is left.
16. As a script author, I want any other non-zero select exit to stop the run with an error that shows select's stderr, so that a broken selector never looks like an empty plan.
17. As a script author, I want precheck exiting `0` to skip work, verify and judge and go straight to commit as already done, so that work already satisfied at the base commit costs no agent call.
18. As a script author, I want any non-zero precheck exit, including a kill by signal, to mean the task needs work, so that precheck has no failure states of its own.
19. As a script author, I want each step's stdout piped to the next step's stdin, so that steps can hand summaries forward without files, as the Script Bridge does today.
20. As an operator, I want a `work.sh` failure, crash or kill by signal to roll back and continue with the next iteration, so that one bad agent run doesn't end an unattended loop.
21. As a script author, I want `work.sh` exiting `3` to mean declined, so that the agent can report that its task is wrong.
22. As an operator, I want a declined iteration to roll back and then end the loop as `Declined` with the task, so that the plan is fixed before more work is built on it.
23. As a script author, I want verify to run only after work succeeds, and judge only after verify passes, so that slow, non-deterministic checks never run on code that fails cheap ones.
24. As a script author, I want any non-zero verify or judge exit, including a kill by signal, to roll back, so that nothing that fails a check is committed.
25. As a script author, I want commit and rollback to receive `SHIFT_OUTCOME`, so that one script can record each outcome differently.
26. As a script author, I want rollback's stdin to carry the failing step's report, so that it can record why the attempt failed.
27. As an operator, I want a non-zero exit from commit or rollback to stop the run with an error, so that the loop never continues from an unknown repository state.
28. As an operator, I want every script's stderr streamed to my terminal as it is written, so that I can watch an agent work as I would with Farr's loop.
29. As an operator, I want each step's stdout and stderr also saved in the iteration directory, so that I can inspect any iteration afterwards.
30. As an operator, I want each run to get its own run directory inside the repository's git directory, so that logs are never committed, never show up as changes, and survive rollback's clean.
31. As an operator, I want each iteration directory to record the loop, task, base commit and outcome, so that a run can be reconstructed and measured later.
32. As an operator, I want a progress line on stderr for each step and outcome, so that I can follow the loop without reading transcripts.
33. As an operator, I want each loop to end with a summary counting iterations, commits, already-done tasks, rejections by outcome and declines, so that I can judge how a run went.
34. As an operator, I want the end state printed as the last stdout line and a zero exit status for every clean end state, so that scripts that call shift can read the result.
35. As an operator, I want the review stage to run `.shifting/review/select.sh` and, when it reports pending items and a terminal is attached, run `review.sh` with my terminal handed to it, so that I can hold an interactive session with an agent.
36. As an operator, I want an unattended run to skip the interactive session and print the pending items instead, so that nothing waits for input that will never come.
37. As an operator, I want an override for terminal detection (`auto`, `always`, `never`), so that tests and CI can force either path.
38. As an operator, I want review's `commit.sh` to run after a successful session, and a failing session to stop the run with my edits left in place, so that half-made spec edits are never committed silently.
39. As an operator, I want a shift run to start with review when items are pending, so that planning uses specs I have just corrected.
40. As an operator, I want a shift run to run the plan loop until it is exhausted before building, so that building always starts from a settled plan.
41. As an operator, I want a build loop that ends `Declined` to send the run back to the plan loop and then resume building, so that a wrong plan is corrected without my intervention.
42. As an operator, I want a plan loop that ends `Declined` to end the run and report the task, so that a planner that can't proceed comes back to me.
43. As an operator, I want a shift run to finish with a review and, when that review commits changes, to plan and build again, so that spec corrections are acted on in the same run.
44. As an operator, I want a shift run to end `Done` only when the build loop ended `Exhausted` and the last review committed nothing, so that `Done` always means no work is left.
45. As an operator, I want `--max-iterations` on a shift run to bound the plan and build iterations of the whole run together, so that one number caps an experiment.
46. As an operator, I want Ctrl+C to stop shift immediately, so that I stay in control. The next run refuses to start until I clean up any leftover changes.
47. As a developer, I want the prototype five-script default loop, its `--dir` option and the repository's stub `.shifting/` directory removed, so that only one loop exists.
48. As a developer, I want the loop composed from Step, Branch and Executor with typed states, so that illegal transitions are rejected at compile time.
49. As a developer, I want an interactive script adapter alongside `ScriptStep` and `ScriptBranch`, so that other compositions can also hand the terminal to a script.
50. As a developer, I want the routing from exit codes to outcomes, from outcomes to the next state, and from loop end states to the next shift-run stage to be pure functions, so that the routing rules are tested without spawning processes.

## Implementation Decisions

### 1. Removal of the prototype default loop
- Removed: `DefaultShiftLoop`, `PlanBranch`, `VerifyResolution`, `VerifyAndResolve`, discovery of the five scripts, the `--dir` and `--verbose` options, the repository-root `.shifting/` stub scripts, and the tests that exercise them.
- No compatibility shims, deprecation paths or migration of the old convention.
- The category-theory core (Step, Branch, Either, LoopFlow, Executor and their identities, injections and combinators) and the Script Bridge are kept and reused.

### 2. Repository preconditions
- The repository root is resolved through git. Every command fails with a typed error when:
  - the root is not inside a git work tree;
  - HEAD has no commit;
  - the working tree has staged, unstaged or untracked changes. Ignored files don't count.
- The checks run once, before any script. Between iterations the scripts keep the tree consistent; the runtime doesn't re-check it.

### 3. Loop directories
| Directory | Required scripts |
|---|---|
| `.shifting/plan/` | `select.sh`, `precheck.sh`, `work.sh`, `verify.sh`, `judge.sh`, `commit.sh`, `rollback.sh` |
| `.shifting/build/` | the same seven |
| `.shifting/review/` | `select.sh`, `review.sh`, `commit.sh` |

- Each command checks that the directories it uses exist and contain every required script, and reports the first missing path, before running anything.
- A shift run checks all three directories.

### 4. Script contract
| Script | Stdin | Exit status → meaning |
|---|---|---|
| `select.sh` | empty | `0` selected (trimmed stdout becomes `SHIFT_TASK`) · `2` no work left · other → run stops with error |
| `precheck.sh` | task reference | `0` already done → commit · anything else → needs work |
| `work.sh` | task reference | `0` done (stdout is the work summary) · `3` declined (stdout is the reason) · anything else → rollback |
| `verify.sh` | work summary | `0` pass · anything else → rollback |
| `judge.sh` | work summary | `0` pass · anything else → rollback |
| `commit.sh` | judge stdout, or precheck stdout when already done | `0` · anything else → run stops with error |
| `rollback.sh` | failure report | `0` · anything else → run stops with error |
| review `select.sh` | empty | `0` items pending (stdout lists them) · `2` nothing pending · other → error |
| review `review.sh` | inherited terminal | `0` → review `commit.sh` · anything else → run stops, tree left as is |
| review `commit.sh` | empty | `0` · anything else → error |

- **Signals.** A step killed by a signal counts as "anything else". For select, commit and rollback that means an error.
- **Failure report.** The report passed to rollback is the failing step's stderr, or its stdout when stderr is empty, matching the current Script Bridge behaviour. For a decline it is work's stdout.
- **Encoding.** Stdout must be valid UTF-8; invalid stdout counts as that step failing. Stderr is decoded lossily, since it is for humans.

### 5. Environment
Every script runs with the repository root as its working directory and inherits shift's environment, plus:

| Variable | Value |
|---|---|
| `SHIFT_LOOP` | `plan`, `build` or `review` |
| `SHIFT_ITERATION` | iteration number within the run, counted across loops |
| `SHIFT_TASK` | select's trimmed stdout (unset for select itself) |
| `SHIFT_BASE` | HEAD commit when the iteration started |
| `SHIFT_RUN_DIR` | this run's directory |
| `SHIFT_ITERATION_DIR` | this iteration's directory |
| `SHIFT_OUTCOME` | commit: `verified` or `already-done`; rollback: `work-failed`, `verify-failed`, `judge-failed` or `declined` |

### 6. Typed states and composition
- **States:**
  - `Idle{iteration}`
  - `Selected{task, base}`
  - `WorkDone{task, base, summary}`
  - `Verified{task, base, output}`
  - `Approved{task, base, verdict}`
  - `AlreadyDone{task, base, output}`
  - `Rejected{task, base, outcome, report}`
  - `Declined{task, base, reason}`
- **Branch arms:**
  - Select is a Branch whose right arm ends the loop.
  - Precheck is a Branch: already done, or still `Selected`.
  - Work is a Branch whose right arm separates `Declined` from `Rejected` (work failed).
  - Verify cascades into judge on its left arm.
- `Approved` and `AlreadyDone` go to commit; `Rejected` and `Declined` go to rollback.
- After a commit, or a rollback of a rejection, the iteration yields `LoopFlow::Continue(Idle)`. After a rollback of a decline it yields `LoopFlow::Break(Declined)`.
- The existing combinators are used where they fit. The state types are immutable values consumed by each transition (ADR-0010).

### 7. End states and summaries
- **Loop end:**
  - `Exhausted` — select exited `2`;
  - `Declined{task}`;
  - `MaxIterations`.

  Each comes with a loop summary: iterations, commits, already-done tasks, rejections counted per outcome, and declines.
- **Shift-run end:**
  - `Done`;
  - `Declined{task}` — from the plan loop;
  - `MaxIterations`.

  The run summary totals each loop.
- **Errors**, typed:
  - precondition failures;
  - missing directory or script;
  - select error;
  - commit or rollback failure;
  - review session failure;
  - I/O and Script Bridge errors.
- The last stdout line names the end state. Clean end states exit `0`; errors exit non-zero with the error on stderr.

### 8. Process execution
- The Script Bridge gains environment variables and a working directory per invocation.
- Stderr is copied live to shift's stderr while it is being captured, so no pipe can deadlock.
- Stdout is captured as before.
- Both streams are written to the iteration directory when the step finishes.
- A new interactive adapter runs a script with stdin, stdout and stderr inherited, plus the same environment, and yields its exit status as a Branch. It captures nothing.

### 9. Run directory
- Runs live under a `shifting/runs/` directory inside the repository's git directory, as reported by git. That handles worktrees, keeps runs out of `git status`, and keeps them out of reach of `git clean`.
- Each run has a timestamped directory with one numbered directory per iteration, holding:
  - the loop, task, base commit and outcome;
  - each step's stdout and stderr.
- The run directory also holds the run summary.
- The runtime never deletes run directories.

### 10. Shift run
```
review   (when review select reports pending items)
loop:
  plan loop   → Exhausted: continue · Declined: end run Declined · MaxIterations: end run
  build loop  → Exhausted: continue · Declined: back to plan loop · MaxIterations: end run
  review      → commit moved HEAD: back to plan loop · otherwise: end run Done
```
- When no terminal is attached (or the override says `never`), each review stage prints the pending list and continues as if review committed nothing.
- The iteration bound counts plan and build iterations together across the run.

### 11. CLI
- Subcommands: none (shift run), `plan`, `build`, `review`.
- Global options:
  - the repository root (default: the current directory);
  - `--max-iterations`;
  - the interactive override `auto | always | never`, where `auto` detects whether stdin is a terminal.
- The default-loop helper subcommands and `init` are added by `shifting-default-loop`. clap derive is kept.

### 12. Functional standards
- The pure core holds:
  - the exit status → outcome tables;
  - the state transitions;
  - summary accumulation;
  - the shift-run stage transitions.
- Process spawning, stream copying, git queries and file writes sit at the shell, per ADR-0010 and the crate's strict Clippy lints.

## Testing Decisions

### What Makes a Good Test
Tests check observable behaviour:
- which scripts ran, in which order;
- with which environment and stdin;
- what ended up in the repository and the run directory;
- which end state and summary were reported, and what the CLI printed.

They never assert on private types or internal call sequences.

### Seam 1: The `shift` CLI against a temporary git repository
- **Location.** The CLI entry point and the built binary, run against a temporary git repository with an initial commit.
- **Setup.** Stub scripts in `.shifting/plan/`, `.shifting/build/` and `.shifting/review/` append their name, environment and stdin to a log file. They exit with scripted codes, which can depend on `SHIFT_TASK` or a counter file.
- **Scenarios:**
  1. A build loop over three tasks:
     - one commits;
     - one fails verify and rolls back;
     - one fails judge and rolls back;
     - select then exits `2`.

     The log shows the step order, `SHIFT_OUTCOME` values, rollback stdin, the summary counts and `Exhausted`.
  2. Precheck exits `0`: work, verify and judge never run, and commit receives `already-done`.
  3. Work exits `1`, and separately is killed by a signal: rollback runs with `work-failed` and the loop continues.
  4. Work exits `3`: rollback runs with `declined` and the reason on stdin, and the loop ends `Declined{task}`.
  5. Select exits `1`, and separately commit exits `1`: the run stops with an error and a non-zero exit status.
  6. Preconditions: a dirty tree, an untracked file, a non-repository and a repository with no commits are each refused before any script runs. A missing script is reported by its path.
  7. `--max-iterations`, including `0`, ends `MaxIterations` with exact counts.
  8. Stderr written by a long-running stub appears on shift's stderr before the stub exits, and is saved in the iteration directory along with stdout.
  9. Run directories:
     - they are created inside the git directory;
     - `git status` stays clean;
     - `git clean -fd` leaves them in place.
  10. A shift run:
      - planning precedes building;
      - a build `Declined` returns to planning;
      - a plan `Declined` ends the run;
      - a final review commit (with the interactive override `always` and a stub `review.sh`) triggers another plan and build round;
      - the run ends `Done` when review commits nothing.
  11. Interactive override `never`: `review.sh` doesn't run and the pending list is printed.
  12. `review.sh` exits non-zero: the run stops and review's `commit.sh` doesn't run.
  13. CLI parsing of subcommands and global options.

### Seam 2: Pure routing functions
- **Location.** The pure functions in section 12, called with in-memory values.
- **Checks:**
  - every exit status in the contract table maps to the right outcome;
  - a sequence of outcomes accumulates the right summary;
  - every loop end state leads to the right next shift-run stage.

### Existing Seams
The Step, Branch and ADT seams from `shifting-orchestrator` stay as they are. The Script Bridge tests gain cases for environment variables, the working directory, live stderr and the interactive adapter.

### Prior Art
- `packages/shifting/tests/shift_loop_tests.rs`: stub scripts in temporary directories, binary execution through `CARGO_BIN_EXE_shift`, iteration bounds.
- `packages/shifting/tests/script_tests.rs`: stdin/stdout transfer, signals, large payloads without deadlock, broken pipes.

Tests and Clippy run with warnings denied across all targets, per `docs/agents/validation.md`.

## Out of Scope
- The shipped scripts, prompts, task and assumption formats, helper subcommands and `shift init` (see `shifting-default-loop`).
- The sandboxed playground image (see `shifting-playground`).
- Attempt budgets, timeouts, stall detection, revise loops or any other limit. Version 1 stops only on select exiting `2`, a decline, `--max-iterations`, an error or Ctrl+C.
- Signal handling or cleanup on Ctrl+C.
- Pushing, tagging, or any remote interaction.
- Running several loops at once, or one loop across worktrees.
- Non-Unix platforms.

## Further Notes
- **Decisions.** Implements ADR-0011 as amended. The spec ownership rules that the shipped scripts enforce are recorded in ADR-0012.
- **Supersedes.** Replaces the default-loop parts of `shifting-orchestrator`: the five-script convention, stories 11 and 12, and the state topology diagram. That spec's category-theory stories still hold.
- **Order.** Build this spec first. `shifting-default-loop` depends on it, and `shifting-playground` depends on both plus Nix packaging (`shifting-orchestrator` issue 05).
- **References.** Farr's loop: https://github.com/ClaytonFarr/ralph-playbook. Huntley's original: https://ghuntley.com/ralph/.
- **Vocabulary.** Terms are defined in `GLOSSARY.md`.
