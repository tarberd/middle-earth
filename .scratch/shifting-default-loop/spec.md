# Shifting Default Loop: Farr's Ralph Loop with Human-Owned Topic Specs

Status: ready-for-agent

## Problem Statement

An operator who wants to run Clayton Farr's Ralph loop on a codebase, whether fresh or populated, has to copy prompts and a bash script by hand. They then inherit the loop's weaknesses:

- **Self-reported work.** The agent picks its own task, decides when it is done, and commits and pushes. A false claim of success lands in git, and nothing rolls back work that fails its tests.
- **No spec checkpoint.** When the agent finds an inconsistency, it edits the specs itself, so the specs drift toward whatever the code does without a human ever deciding. Huntley's alternative, where the human fixes specs, leaves the agents' guesses invisible until something breaks.
- **AGENTS.md.** Farr's operational notes live in `AGENTS.md`. In a populated codebase that file already belongs to the humans, and every agent loads it, including agents that shouldn't see agent-written notes.
- **Free-form plan.** `IMPLEMENTATION_PLAN.md` is free text, so nothing deterministic can choose the next task, check the plan, or tell a stale plan from a current one.

## Solution

`shift init` writes shifting's only loop into a repository: Farr's plan and build loops and prompts, adapted so that the harness owns everything an agent could misreport.

- **Topics.** Each topic of concern is a directory under `spec/` holding the following.

  | Item | What it is |
  |---|---|
  | `spec.md` | The topic spec. Only humans write it, in any format they like; shifting imposes none. |
  | `plan/` | A disposable directory of numbered plan tasks, plus the content hash of the spec it was planned from. |
  | `assumptions/` | Durable records of interpretations agents chose where the spec is silent or contradicts itself, kept until a human reviews them. |

- **The plan loop.**
  - It regenerates a topic's plan whenever the spec's content changes, running the spec gate first.
  - It rewrites tasks the builder declined.
- **The build loop.**
  - Selection is deterministic: the lowest-numbered eligible task.
  - Deterministic checks run first, followed by project checks the operator fills in, then a passive LLM judge.
  - The harness commits each green iteration. A failure rolls back to the base commit and keeps only useful notes.
- **What agents may do.** Implement, record assumptions, add discovered tasks, decline wrong tasks, and keep the operations notes up to date.
- **What agents may not do.** Commit, push, tag, edit a topic spec, change a task's status, or touch `.shifting/`.
- **Review.** An interactive session in which the operator accepts, corrects or dismisses each assumption. It is the only place an agent edits a topic spec, and only as the operator directs.
- **Operations notes.** Farr's `AGENTS.md` moves to a separate file. Only the plan and build prompts load it.
- **Thin scripts.** The deterministic logic is pure Rust, exposed as `shift` helper subcommands. The shipped scripts stay thin bash that the operator can edit.
- **Agent commands.** Every agent command can be overridden, which lets tests use a fake agent and lets operators use other CLIs or models.

## User Stories

### Setup
1. As an operator, I want `shift init` to write the default plan, build and review scripts and prompts into `.shifting/`, so that I can start a Ralph loop without copying files by hand.
2. As an operator, I want `shift init` to leave existing files untouched and list what it skipped, so that running it in a populated repository never overwrites my files.
3. As an operator, I want `shift init` to write an operations notes template with Farr's sections, so that agents have a place for what they learn about running the project.
4. As an operator, I want `shift init` not to create, require or validate any topic spec format, so that I write specs with my own tools.
5. As an operator, I want `shift init` to print what is left for me to do (fill in the project checks, write topic specs, commit), so that I know what's needed before the first run.
6. As an operator, I want build select to stop the run with a clear message while the project checks are still unconfigured, so that an unconfigured loop never calls an agent or commits unchecked code.
7. As an operator, I want the same layout and scripts to work in a fresh repository with no code yet, so that greenfield and populated codebases use one loop.
8. As an operator, I want the planner, builder, judge and review agent commands to be overridable through environment variables, with the planner and builder defaulting to Farr's `claude -p --dangerously-skip-permissions --output-format=stream-json --model opus --verbose`, so that I can use another CLI or model.
9. As an operator, I want every shipped script and prompt to be a plain editable file, so that I can change any behaviour for my project.
10. As an operator, I want the shipped files to warn that the default agent command skips permission prompts and must run in a sandbox, so that nobody runs it unprotected by accident.

### Topics and specs
11. As a spec author, I want each topic of concern to be a directory under `spec/` holding exactly one `spec.md`, so that one topic maps to one spec, as in Farr's playbook.
12. As a spec author, I want both loops to reject any iteration that changes a topic spec, so that the spec stays the fixed point agents work toward.
13. As a spec author, I want a topic replanned whenever its spec's content changes, detected by content hash rather than by commit, so that rebases and amends don't trigger replanning but real edits do.

### Plan loop
14. As an operator, I want a topic's plan to be a directory of numbered task files, so that each task can be selected, checked and marked on its own.
15. As an operator, I want each task to record its status (`open`, `done` or `declined`), its blockers, the parts of the spec it covers, and acceptance criteria that each name tests, so that selection and checks are deterministic.
16. As an operator, I want the task number to be the build order, so that the planner's prioritisation is visible and selection is deterministic.
17. As an operator, I want deleting a topic's plan directory to make the next plan loop regenerate the plan from the spec and the code, so that a wrong plan costs one planning pass, as Farr intends.
18. As an operator, I want the plan loop to delete and regenerate a topic's plan when the spec's content hash differs from the one recorded in the plan, so that a plan never outlives the spec it was made from.
19. As an operator, I want the plan loop to pick one topic per iteration, in name order, from those that have no plan, a changed spec or a declined task, and to end when none is left, so that planning settles deterministically.
20. As an operator, I want the spec gate to review a topic in a fresh context before it is planned from scratch, recording as assumptions any contradictions, questions a builder would have to ask, behaviours without an observable outcome, and topics that fail Farr's "one sentence without *and*" test, so that spec problems surface before code is built on them.
21. As a planner agent, I want to study the topic spec, its assumptions, the operations notes, the existing plan (which may be wrong) and the code, and to confirm something is missing before planning it, so that a populated codebase isn't replanned from nothing.
22. As a planner agent, I want to derive each task's required tests from what the spec says must be true, so that backpressure is acceptance-driven, as in Farr's enhancement.
23. As a planner agent, I want to rewrite, split or delete every declined task with its reason in view, so that the plan is corrected exactly where the builder found it wrong.
24. As a planner agent, I want to search for TODOs, placeholders, skipped tests and inconsistent patterns, so that unfinished work in a populated codebase gets planned.
25. As a planner agent, when the spec is silent or contradicts itself, I want to record an assumption and plan according to it, so that planning never blocks on a question.
26. As an operator, I want plan verify to reject an iteration that:
    - touches anything outside the topic's plan and new assumption files;
    - edits a done task;
    - leaves a declined task;
    - adds a task that isn't open;
    - breaks the task format;
    - references a blocker that doesn't exist;
    - or creates a blocker cycle,

    so that a bad plan never lands.
27. As an operator, I want the plan commit to record the spec's content hash in the plan and commit the plan, so that staleness can be detected and a rollback can't wipe an uncommitted plan.

### Build loop
28. As an operator, I want the build loop to pick the lowest-numbered open task whose blockers are all done (topics in name order), and to end only when no open task remains, so that the agent never picks its own work and the loop never finishes with work left.
29. As an operator, I want build select to stop the run with an error naming the cause when a topic still needs planning, or when open tasks remain but none is eligible, so that a stale or stuck plan is never built on or reported as finished.
30. As an operator, I want precheck to mark a task done without calling the agent when every test it names already passes at the base commit, so that work that is already implemented costs nothing.
31. As a builder agent, I want my prompt to give me the task, the topic spec, its assumptions, the operations notes and the task's notes from earlier attempts, so that each fresh context starts with what it needs.
32. As a builder agent, I want to keep Farr's working rules, so that the loop keeps what makes Ralph work:
    - search before assuming something is missing;
    - use many subagents for reading but only one for building and testing;
    - implement completely, without placeholders;
    - record why each test exists;
    - keep a single source of truth.
33. As a builder agent, I want to run the build verify script myself before finishing, so that I see the same checks that will decide my work.
34. As a builder agent, when the spec is silent or contradicts itself, I want to choose an interpretation, record it as an assumption and carry on, so that I never block on a question.
35. As a builder agent, I want to follow existing assumptions, so that consecutive fresh contexts stay consistent until a human decides.
36. As a builder agent, I want to add discovered work as new open tasks in the right topic, so that bugs I notice aren't lost, as in Farr's loop.
37. As a builder agent, I want to decline a task that is wrong by adding a declined section with my reason to it and stopping, so that the plan gets fixed instead of me forcing a bad task through.
38. As a builder agent, I want to record what I learn about building and running the project in the operations notes, briefly and operational only, so that later iterations don't rediscover commands.
39. As an operator, I want build verify to check, cheapest first, that the iteration:
    - changed something;
    - didn't move HEAD;
    - didn't touch `.shifting/` or any topic spec;
    - changed the plan only by adding open tasks;
    - only added assumption files;
    - kept every new file well-formed;
    - and passes the project checks and every test the task names,

    so that only honest, checked work goes further.
40. As an operator, I want the judge to see only the topic spec, its assumptions, the task and the diff, in a fresh context using a different model from the builder, and never the transcript or the operations notes, so that it judges the work rather than the agent's account of it.
41. As an operator, I want the judge to check that the acceptance criteria are really met rather than gamed, that the work is consistent with the spec and its assumptions, and that it follows the project standards I list, so that problems deterministic checks miss are caught.
42. As an operator, I want the judge to ship passive, logging its verdict without failing the iteration, with one switch to make it a gate, so that I can calibrate it before trusting it.
43. As an operator, I want the build commit to mark the task done and commit everything, with a message naming the topic and task and with no push or tag, so that every green iteration lands as one commit I can push myself.
44. As an operator, I want a rollback to keep useful memory and nothing else, so that a failed attempt teaches the next one. A rollback should:
    1. reset to the base commit and remove untracked files, but not ignored ones;
    2. restore any well-formed new assumptions, new tasks and operations-notes changes the agent wrote;
    3. append the outcome and the tail of the failure report to the task's notes;
    4. commit that.
45. As an operator, I want a declined task marked declined with its reason after the rollback, so that the plan loop knows what to fix.
46. As an operator, I want `work.sh` to save the agent's full transcript in the iteration directory, stream it live to stderr and print only a short summary on stdout, so that logs are complete without flooding the Script Bridge.

### Assumptions and review
47. As an operator, I want each assumption to record who filed it, what the spec says (nothing, or the conflicting passages), what was assumed, what relies on it, and the proposed spec text, so that I can decide quickly.
48. As an operator, I want gaps and conflicts recorded as the same kind of assumption, so that agents never have to classify them.
49. As an operator, I want assumptions never to block selection, planning or building, so that no loop stalls waiting for my review.
50. As an operator, I want assumptions to survive plan regeneration and rollbacks until I review them, so that no agent guess is ever lost.
51. As an operator, I want review to list pending assumptions across topics and then, in an interactive session, walk me through each one so that I can accept, correct or dismiss it. The agent edits the spec only as I direct and deletes the assumption once I have decided, so that every guess ends up in the spec or is discarded by me.
52. As an operator, I want my review edits committed, so that the changed spec hashes trigger the spec gate and replanning.

### Operations notes and helpers
53. As an operator, I want operations notes kept outside `AGENTS.md` and `CLAUDE.md` and loaded explicitly by the plan and build prompts only, so that my instruction files stay mine and the judge and spec gate never see agent-written notes.
54. As a script author, I want the deterministic logic available as `shift` subcommands, so that the shipped scripts stay thin and replaceable while the logic is pure, tested Rust. That covers:
    - choosing the next task and the next topic;
    - plan and assumption format checks and the blocker graph;
    - diff rules;
    - task status and notes;
    - named tests;
    - pending assumptions.

## Implementation Decisions

### 1. Repository layout written and read by the default loop
| Path | Written by | Lifetime | Holds |
|---|---|---|---|
| `spec/<topic>/spec.md` | humans (in review: an agent, as directed) | permanent | the topic spec, in any format |
| `spec/<topic>/plan/<NN>-<slug>.md` | planner; builder adds open tasks; harness sets status and notes | disposable | one plan task |
| `spec/<topic>/plan/planned-against` | plan commit only | disposable | git blob hash of `spec.md` when the plan was made |
| `spec/<topic>/assumptions/<NN>-<slug>.md` | builder, planner, spec gate; deleted in review | until reviewed | one assumption |
| `docs/agents/operations.md` | builder | permanent | operations notes |
| `.shifting/{plan,build,review}/` | operator | permanent | scripts and prompts |

- A topic exists exactly when `spec/<topic>/spec.md` exists, and its name is its directory name.
- Task and assumption numbers have at least two digits, are unique within the topic's directory, and sort numerically.
- A task is referenced as `<topic>/<NN>`.

### 2. Plan task format
- **Required lines:**
  - a `# <title>` heading;
  - a `Status:` line (`open | done | declined`);
  - a `Covers:` line in free text naming the parts of the spec the task implements.
- **Optional line:** `Blocked by:`, listing `NN` in the same topic or `<topic>/NN` in another.
- **Sections:**
  - `## Description`;
  - `## Acceptance criteria`, a list of at least one item, in which every item names at least one test in backticks. A task with no named test is malformed, so precheck can never mark it done vacuously;
  - `## Notes`, appended by the harness only;
  - `## Declined`, written by the builder only.
- Status changes are made only by the harness.
- A new task is always `open`.
- A task is eligible when it is `open` and everything it is blocked by is `done`.

### 3. Assumption format
- **Heading:** `# <title>`.
- **`Filed by:`** `builder`, `planner` or `spec-gate`, with the task or topic.
- **Sections:**
  - `## Spec says`: "nothing", or one or two quoted passages;
  - `## Assumed`;
  - `## Relied on by`;
  - `## Proposed spec text`.
- Gaps and conflicts share this one format, with no type field.
- Once written, an assumption is never edited by an agent. Only review removes it.

### 4. Staleness and regeneration
- Staleness is decided by content alone, using the git blob hash of the topic's `spec.md`.
- The plan commit writes the current hash to `planned-against`; nothing else writes it.
- A topic needs planning when any of these holds:
  - its plan directory or `planned-against` is missing;
  - the recorded hash differs from the current one;
  - it has a declined task.
- **Stale or missing plan.** Plan `work.sh` deletes every file in the topic's plan directory, runs the spec gate, then runs the planner.
- **Declined task.** The planner updates the existing plan instead.
- **Manual regeneration.** The operator deletes the topic's plan directory and commits. There is no flag for it.

### 5. Helper subcommands (pure core, imperative shell)
| Subcommand | Used by | Behaviour |
|---|---|---|
| `shift tasks next` | build select | prints `<topic>/<NN>` of the first eligible task; exits `2` when no open task remains; exits `1` with the cause when a topic needs planning or open tasks remain but none is eligible |
| `shift tasks tests <task>` | precheck, verify | prints the test names from the task's acceptance criteria, one per line |
| `shift tasks set-status <task> <status>` | commit, rollback | rewrites the status line |
| `shift tasks note <task>` | rollback | appends stdin under `## Notes` |
| `shift topics next` | plan select | prints the first topic needing planning; exits `2` when none does |
| `shift assumptions list` | review select | lists pending assumptions; exits `2` when none exist |
| `shift lint [<topic>]` | both verifies, rollback | checks task and assumption formats, that blocker references resolve across topics, and that the blocker graph has no cycles |
| `shift check-diff build <task>` / `plan <topic>` | verify | checks the changes against `SHIFT_BASE`, untracked files included and ignored files excluded, using the rules in sections 7 and 8 |

- Files are parsed into typed task and assumption values (parse, don't validate).
- Selection, linting and diff-rule evaluation are pure functions over those values and over a typed list of path changes.
- Reading files and querying git happen at the shell (ADR-0010).
- Violations are reported one per line with the path and the rule broken.

### 6. Project checks
- `.shifting/build/project.sh` is the only file the operator must fill in. It defines three functions:
  - **`project_configured`.** The shipped version reports false, with instructions for filling the file in.
  - **`project_checks`.** The project's placeholder scan, lint and tests. It ships with commented examples for common languages.
  - **`run_named_test <name>`.** Runs one named test.
- Build select, precheck and verify source this file.
- Build select stops the run while `project_configured` reports false.
- It plays the role of the build, lint and test commands in Farr's AGENTS.md template.

### 7. Plan loop scripts (`.shifting/plan/`)
| Script | Behaviour |
|---|---|
| `select.sh` | `shift topics next` |
| `precheck.sh` | always "needs work" |
| `work.sh` | if the plan is stale or missing: delete it and run the spec gate (`GATE.md`) through the planner command; then run the planner (`PROMPT.md`) with the topic named; save both transcripts in the iteration directory |
| `verify.sh` | `shift check-diff plan <topic>` and `shift lint` |
| `judge.sh` | pass-through, editable by the operator |
| `commit.sh` | write `planned-against`, then commit the plan and new assumptions as `plan(<topic>): …` |
| `rollback.sh` | reset to the base commit and clean; restore new assumptions that pass lint; commit them if any |

`check-diff plan` allows:
- changes only inside the topic's plan directory, plus new files in any topic's `assumptions/`;
- deleting a done task, but never editing one;
- new tasks only when they are `open`;
- deleting `planned-against`, but not writing it.

After the change, the topic must have no declined task.

### 8. Build loop scripts (`.shifting/build/`)
| Script | Behaviour |
|---|---|
| `select.sh` | stop if the project is unconfigured; otherwise `shift tasks next` |
| `precheck.sh` | already done when every name from `shift tasks tests` passes `run_named_test`; otherwise needs work |
| `work.sh` | put the task's path before `PROMPT.md`, run the builder command, and save the transcript while streaming it to stderr; exit `3` and print the reason when the task gained `## Declined`; otherwise print a short summary |
| `verify.sh` | in order: `shift check-diff build <task>`, `shift lint`, `project_checks`, `run_named_test` for each named test |
| `judge.sh` | run `JUDGE.md` through the judge command with the topic spec, its assumptions, the task and the full diff; save the verdict in the iteration directory; passive by default (always passes), gate when `SHIFT_JUDGE=gate` (fails on a `FAIL` verdict, with its feedback) |
| `commit.sh` | `shift tasks set-status <task> done`, stage everything, commit as `<topic>/<NN>: <title>` (or `… already satisfied`) |
| `rollback.sh` | see below |

**Rollback steps:**
1. Set aside the agent's new task and assumption files and its operations-notes changes.
2. Run `git reset --hard $SHIFT_BASE` and `git clean -fd`.
3. Restore whatever was set aside that passes lint.
4. For a declined task, set its status to `declined` and note the reason. For any other outcome, note the outcome and the tail of the report; the full report stays in the run directory.
5. Commit as `<topic>/<NN>: <outcome>`.

`check-diff build` rejects an iteration when:
- the change set is empty or HEAD moved;
- it touches `.shifting/` or any topic spec;
- it changes or deletes an existing task or `planned-against`;
- it adds a task that isn't `open`;
- it changes or deletes an existing assumption.

Code, documentation and the operations notes may change freely.

### 9. Review scripts (`.shifting/review/`)
| Script | Behaviour |
|---|---|
| `select.sh` | `shift assumptions list` |
| `review.sh` | start the review command interactively with `PROMPT.md` and the pending list |
| `commit.sh` | when anything changed, stage everything and commit as `review: …`; otherwise do nothing |

The review prompt goes through each pending assumption and asks the operator to:
- **accept**: write the proposed text into the spec;
- **correct**: write the operator's answer into the spec instead;
- **dismiss**: discard it, clarifying the spec if the operator wants.

The agent edits topic specs only as the operator directs, and deletes each assumption once it is decided.

### 10. Prompts (Farr's, adapted)
- **Plan `PROMPT.md`:**
  - **Study:** the topic spec, the assumptions, the operations notes, the existing plan ("it may be incorrect") and the code.
  - **Confirm before planning:** check that something is missing with a code search before planning it.
  - **Plan tasks:** small enough for one iteration, numbered in build order, with blockers, a `Covers:` line and acceptance criteria that name tests derived from the spec.
  - **Clean up:** resolve every declined task; look for TODOs, placeholders, skipped tests and inconsistent patterns.
  - **Record:** assumptions for gaps and conflicts.
  - **Plan only:** no code changes, no spec edits, no commits.
- **Plan `GATE.md`:**
  - **Inputs:** the topic spec, the other topic specs and the existing assumptions.
  - **Records assumptions** for:
    - contradictions;
    - questions a builder would have to ask;
    - behaviours without an observable outcome;
    - a topic that can't be described in one sentence without *and*. In that case the proposed spec text splits the topic.
  - **Writes** only assumptions.
- **Build `PROMPT.md`:**
  - **Kept from Farr:** search first; parallel subagents for reading and one for building and testing; implement completely, without placeholders; capture why each test exists; keep a single source of truth; fix unrelated failing tests as part of the increment.
  - **Context:** the task, the topic spec and its assumptions, the operations notes, and the task's notes from earlier attempts.
  - **Before finishing:** run `.shifting/build/verify.sh`.
  - **Recording:** assumptions instead of spec edits (follow existing ones); discoveries as new open tasks; what was learned about running the project in the operations notes, operational only.
  - **Declining:** add `## Declined` with the reason, then stop.
  - **Never:** commit, push, tag, change task statuses, or touch `.shifting/` or topic specs.
- **Build `JUDGE.md`:**
  - **Inputs:** the topic spec, its assumptions, the task and the diff only.
  - **Answers:** whether each acceptance criterion is met by behaviour rather than gamed (hard-coded results, weakened or deleted assertions); consistency with the spec and its assumptions; the project standards listed in the prompt (left for the operator to fill in).
  - **Output:** `PASS` or `FAIL` on the first line, then feedback.
- **Review `PROMPT.md`:** as in section 9.
- **Dropped from Farr's prompts:**
  - picking "the most important item";
  - commit, push and tag instructions;
  - spec edits on inconsistency;
  - the fixed subagent counts;
  - the `999…` rule numbering.

### 11. Operations notes
- The template has Farr's sections: Build & Run, Operational Notes, Codebase Patterns.
- Its header tells agents three things:
  - it is operational only;
  - status and progress belong in the plan;
  - validation means running the build verify script.
- The plan and build prompts include it explicitly. The gate, judge and review prompts don't.

### 12. Agent commands
| Variable | Used by | Default |
|---|---|---|
| `SHIFT_AGENT` | planner, spec gate, builder | `claude -p --dangerously-skip-permissions --output-format=stream-json --model opus --verbose` |
| `SHIFT_JUDGE_AGENT` | judge | the same CLI in print mode with a different model from the builder |
| `SHIFT_REVIEW_AGENT` | review | the same CLI, interactive |
| `SHIFT_JUDGE` | judge | `passive` (alternative: `gate`) |

Each command reads its prompt on stdin. The review command instead receives it as its initial message.

### 13. `shift init`
- Writes the files in sections 6–11, embedded in the binary, and creates `spec/` if it is missing.
- The embedded files' sources live in the crate's `share/` subfolder (`packages/shifting/share/`), laid out as `shift init` writes them.
- Never overwrites existing files: it lists what it wrote and what it skipped.
- Never commits.
- Never writes `AGENTS.md`, `CLAUDE.md` or any topic spec.
- Works in a repository with or without existing code. It needs a git repository, but not a commit or a clean tree, so it can run before the first commit.

## Testing Decisions

### What Makes a Good Test
Tests check what an operator would observe:
- commits and their messages;
- task statuses and notes;
- which assumption files exist;
- the working tree after a rollback;
- end states and helper output.

They don't assert on parser internals or on the order of file reads.

### Seam 1: The `shift` CLI with the shipped scripts and a fake agent
- **Location.** The built binary, run against a temporary git repository prepared with `shift init`, a filled-in `project.sh` whose checks and named tests are fixture commands, and fixture topic specs.
- **Fake agents.** `SHIFT_AGENT`, `SHIFT_JUDGE_AGENT` and `SHIFT_REVIEW_AGENT` point to fake agent scripts. Each reads the prompt and applies scripted edits keyed on the task or topic it names.
- **Scenarios:**
  1. `shift init` in an empty and in a populated repository: files written, existing files skipped, nothing committed. Build select stops while `project.sh` is unconfigured.
  2. A plan loop on a never-planned topic:
     - the gate runs before the planner;
     - tasks and assumptions are committed;
     - `planned-against` matches the spec's blob hash;
     - a second plan loop selects nothing.
  3. Editing the spec and committing regenerates the plan. Rebasing or amending without changing the spec's content doesn't.
  4. Plan verify rejects, and plan rollback keeps only well-formed new assumptions, for each of:
     - a planner that edits the spec;
     - one that edits a done task;
     - one that leaves a declined task;
     - one that creates a blocker cycle.
  5. A build loop:
     - tasks are committed in number order;
     - a blocked task waits for its blocker;
     - every commit marks its task done;
     - the loop ends `Exhausted` when no open task remains.
  6. Precheck: a task whose named tests already pass is committed as already satisfied, without the fake agent being called.
  7. Build verify rejects each of the following, and the rollback leaves the tree at the base commit plus well-formed new assumptions, tasks and notes:
     - a builder that commits;
     - one that touches `.shifting/`;
     - one that edits the spec;
     - one that changes its own task's status;
     - one that adds a malformed assumption;
     - one that breaks a named test.
  8. A decline: the task becomes `declined` with its reason, the build loop ends `Declined`, and a shift run replans the topic and then resumes building.
  9. The judge:
     - passive: a `FAIL` verdict is logged and the iteration is still committed;
     - with `SHIFT_JUDGE=gate`: the same verdict rolls back, with the feedback in the task's notes.
  10. Review: with the interactive override `always` and a fake review agent, accepting an assumption edits the spec, deletes the assumption and commits, and the next plan loop regenerates that topic.
  11. Build select stops with an error when a topic needs planning, and when open tasks remain but none is eligible.

### Seam 2: Pure functions behind the helper subcommands
- **Location.** The task and assumption parsers, selection, lint, the blocker graph and the diff rules, called with in-memory values.
- **Cases**, table-driven:
  - eligibility and ordering across topics;
  - every format violation;
  - blocker references across topics, and cycles;
  - each `check-diff` rule, for build and for plan, as allowed and rejected path changes;
  - the staleness decision.

### Prior Art
- `packages/shifting/tests/shift_loop_tests.rs`: temporary-directory fixtures and binary execution.
- The `shifting-loop-runtime` CLI seam tests, whose stub-script harness these tests extend.

## Out of Scope
- Topic spec templates and checks of spec format.
- Tooling for the job-to-be-done conversation that produces topic specs: the operator's own tools handle it.
- Reverse-engineering specs from an existing codebase.
- Attempt budgets, timeouts, stall detection, revise loops and caps on assumptions or planning passes.
- Pushing, tagging, remotes, pull requests.
- Building several tasks at once.
- Placeholder scans beyond the commented examples in `project.sh`.
- Testing against real agent CLIs. Real-model runs are experiments in `shifting-playground`.

## Further Notes
- **Dependencies.** Depends on `shifting-loop-runtime` for the script contract, outcomes, end states, run directory and shift-run stages.
- **Decisions.** ADR-0011 (amended) records that this is shifting's only loop and that the harness owns selection, verification, commit and rollback. ADR-0012 records spec ownership and assumptions.
- **Departures from Farr:**
  - deterministic selection;
  - harness-owned commit and rollback;
  - no push or tag;
  - assumptions instead of agents editing specs;
  - operations notes instead of `AGENTS.md`;
  - one plan per topic instead of one per project;
  - a structured plan;
  - a spec gate and a passive judge.

  Everything else follows https://github.com/ClaytonFarr/ralph-playbook.
- **Vocabulary.** Terms are defined in `GLOSSARY.md`.
