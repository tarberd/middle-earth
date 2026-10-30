# Category-Theory Type-State Machine for Agentic Orchestration

To eliminate token ballooning, context rot, and unverified model drift across long-running development loops, autonomous agent orchestration is decoupled into an out-of-process, zero-dependency Type-State machine library named `shifting` in `packages/shifting`. The core engine models transitions mathematically using Category Theory primitives: unconditional transformations as morphisms (`Step<In, Out>`), conditional decision points as pure coproduct morphisms (`Branch<In, Left, Right>` yielding zero-dependency `Either<L, R>`), canonical coproduct injections (`either::IntoLeft`, `either::IntoRight`), coproduct identity (`either::Identity`, `either::id()`), sequential identity morphisms (`step::Identity<T>`, `step::id()`), direct cascading branch-to-branch composition (`branch_both`, `branch_left`, `branch_right`), fully heterogeneous error typing across sequential transitions and branch cascades (`Either<ErrA, ErrB>`), and iterative execution driven by control-flow sum types (`LoopFlow<State, Terminal>`) with ergonomic collapses to `Option`. Execution boundaries are bridged via Unix STDIN/STDOUT stream piping through conventional scripts in `.shifting/`, ensuring verification failures deterministically roll back working tree state before code can ever land.

## Amendment (2026-10-30): Farr's Ralph loop as the only loop

**One loop.** The prototype five-script convention (`plan.sh`, `work.sh`, `verify.sh`, `commit.sh`, `rollback.sh`) is removed without compatibility. Shifting ships exactly one loop: Clayton Farr's Ralph loop, modified so that the harness owns everything an agent could misreport, namely task selection, verification, commit and rollback.

**Loop shape.** Plan and build loops share one shape: select, precheck, work, verify, judge, then commit or rollback. Their scripts live in `.shifting/plan/` and `.shifting/build/`. An interactive review stage lives in `.shifting/review/`. A shift run composes them: review, plan until exhausted, build until exhausted, back to planning after a decline, and review again.

**Script context.** Scripts receive context through `SHIFT_*` environment variables while keeping the STDIN/STDOUT convention.

**Outcomes:**
- A work failure rolls back and the loop continues.
- A decline rolls back and ends the loop.
- Only failing commit or rollback scripts stop a run.

**Agents.** Agents never pick their task, commit, push, tag, change task status, or edit `.shifting/`.

**Durability.** The plan is disposable: it can be deleted and regenerated from topic specs and code at any time. Topic specs, assumptions (ADR-0012), operations notes and code are durable.

**Operations notes.** Farr's agent-edited `AGENTS.md` becomes operations notes kept outside the humans' instruction files and loaded only by the plan and build prompts. In populated codebases `AGENTS.md` belongs to humans, different agent CLIs load different files automatically, and the judge and spec gate must not see agent-written notes. Its other job, carrying the test commands, moved into the operator-filled project checks that the harness runs.

**Limits.** Version 1 has no attempt budgets, timeouts or stall detection. A loop with work left stops only on a decline, an optional iteration bound, an error, or Ctrl+C. This follows Farr's own loop, which never stops on its own.

**Helpers.** The default loop's deterministic logic lives in `shift` helper subcommands (pure core, ADR-0010), so the shipped scripts stay thin, editable bash.

**Specs.** Shifting imposes no topic spec format.

**Alternatives.**
- Keeping Farr's loop unmodified as the default was rejected: the agent's own report of success would land in git, and nothing would roll back failing work.
- Shipping it alongside the modified loop as a preset was rejected in favour of a single loop.
