# A duplicated shape is weighed by what it shares

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`docs/agents/assumptions/33-a-site-that-adds-a-step-inside-a-composition-has-its-own-shape.md`: "A site that adds a step between those calls has a shape of its own, and it may keep its own composition."

`code-review` skill baseline: "**Duplicated Code**: the same logic shape appears in more than one hunk or file in the change. → extract the shared shape, call it from both."

## Assumed
Assumption 33's reading holds where the shift loop applies it: commit and rollback share only two calls with the other scripts, around a step of their own. Taken as a general rule, though, any added step would exempt a site from the smell, however much it shares. The smell is weighed by the size of the shared part:
- a site that adds a step may keep its own composition when it shares a call or two;
- when it shares more, the shared composition takes the added step as an argument, and every site calls it.

## Proposed spec text
`code-review` skill baseline, Duplicated Code: "A site that adds a step inside a shared composition may keep its own version only while the shared part is a call or two. Beyond that, the shared version takes the added step as an argument."
