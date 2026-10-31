# A layer that names a shared composition is not a Middle Man

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`docs/agents/assumptions/30-a-layer-that-adds-nothing-to-the-call-is-a-middle-man.md`: "A function delegates onward when all it does is pass its arguments, or values that other functions of its type build from them, to one more function, and its callers already call those functions themselves elsewhere."

`code-review` skill baseline: "**Middle Man**: a class or function that mostly just delegates onward. → cut it, call the real target direct."

## Assumed
Assumption 30 holds that a layer adding nothing is cut, and that stands. Its test, though, would also cut any function that names a composition several sites repeat. That is the extraction `docs/agents/assumptions/29-every-site-of-a-duplicated-shape-uses-the-shared-one.md` asks for, so the two records would undo each other. A layer adds something, and stays, when any of these holds:
- it names a composition that several callers share;
- it binds values that must travel together, such as a script and the error its failure raises;
- it adds data the target receives, or handles the target's result;
- it hides a walk through its own fields.

A Middle Man is a layer that does none of these, so that its operations mostly forward unchanged.

## Proposed spec text
`code-review` skill baseline, Middle Man: "A layer that names a composition several callers share, binds values that must travel together, adds data or handling, or hides a walk through its own fields adds something and stays. Only a layer that does none of these is cut."
