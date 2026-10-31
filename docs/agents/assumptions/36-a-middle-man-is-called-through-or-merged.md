# A Middle Man is called through or merged

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`docs/agents/assumptions/32-a-layer-that-names-a-shared-composition-is-not-a-middle-man.md`, proposed text: "Only a layer that does none of these is cut."

`docs/agents/assumptions/30-a-layer-that-adds-nothing-to-the-call-is-a-middle-man.md`, proposed text: "callers build the values and call the target directly; the layer is merged into its target."

## Assumed
Assumption 32's list of what a layer adds stands. Its proposed text, though, names only one fix for a layer that adds nothing. Either of assumption 30's designs removes such a layer:
- its callers call the target directly;
- the layer is merged into its target.

## Proposed spec text
`code-review` skill baseline, Middle Man: "A layer that adds none of these is removed, either by calling its target directly or by merging it into the target."
