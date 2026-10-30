# Human-Owned Topic Specs and Agent-Recorded Assumptions

**Context.** Ralph loops need backpressure on specs as well as on code. Each topic of concern has one topic spec, and planning and building always aim at it. If agents can change that spec, the loop's fixed point moves toward whatever the code happens to do.

**Who edits topic specs.** Only humans edit topic specs in shifting. Both the plan and build loops reject any iteration that changes a topic spec. The one exception is the interactive review session, where an agent may edit a spec as the human directs.

**Assumptions instead of edits.**
- When a topic spec is silent or contradicts itself, the builder, planner or spec gate chooses an interpretation, records it as an assumption under the topic, and carries on.
- An assumption states what the spec says (nothing, or the conflicting passages), what was assumed, what relies on it, and the proposed spec text.
- Gaps and conflicts are one kind of record. They differed only in whether they blocked work, and nothing blocks now.
- Assumptions never block selection, planning or building.
- Every agent treats them as provisional spec until reviewed, so consecutive fresh contexts stay consistent.
- They survive plan regeneration and rollbacks.
- Review removes each one after the human accepts it (its text goes into the spec), corrects it (the human's answer goes into the spec) or dismisses it.

**Replanning after spec changes.**
- A spec change is detected by the git blob hash of the topic spec, recorded in the plan when it is made. Rebases, amends and squashes don't change that hash; edits do.
- A changed hash regenerates the topic's plan, after a spec gate review.
- The planner then compares the code with the corrected spec and plans any rework. No per-task bookkeeping such as `Assumes:` links is kept.

**Considered alternatives:**
- **Farr's playbook:** agents edit specs when they find inconsistencies. Rejected: specs drift toward the code with no human checkpoint.
- **Huntley's practice:** the human fixes specs, and agents' guesses are never written down. Rejected: an agent's guess stays invisible until it causes a failure.
- **Blocking conflicts, plus a cap on unreviewed assumptions per topic.** Rejected for three reasons:
  - it leaves open tasks the build loop can't finish, which breaks the rule that a build never ends with work left;
  - it needs durable bookkeeping inside a plan that is meant to be disposable;
  - it guards against rework that Ralph accepts as cheap.

**Consequences.** Correcting an assumption late can force rework of code built on it. That cost is accepted, and spec gate reviews before planning reduce it. Shifting imposes no topic spec format, so specs are reviewed by an LLM gate rather than checked against a format.
