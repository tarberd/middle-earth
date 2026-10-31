# A variable the table leaves out is unset, by any design

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`.scratch/shifting-loop-runtime/assumptions/18-an-invocation-names-the-inherited-variables-it-drops.md`, proposed text: "An invocation is a value naming the script, the environment variables added for it, the inherited variables it drops, and its working directory."

`.scratch/shifting-loop-runtime/assumptions/17-variables-scoped-to-some-scripts-are-unset-for-the-rest.md`, proposed text: "A variable the table doesn't give a script is unset for it, even when shift's own environment has it."

## Assumed
Assumption 18's reading stands. Its proposed text, though, fixes one design for the property assumption 17 states. Any invocation that can leave an inherited variable unset satisfies that property, for example:
- a list of the inherited variables it drops;
- environment entries whose value may be "unset";
- a fully explicit environment.

## Proposed spec text
§8: "An invocation names the script, its working directory and how its environment differs from shift's: the variables added and the variables left unset. For example, it may keep a list of dropped names, entries that may be unset, or a fully explicit environment."
