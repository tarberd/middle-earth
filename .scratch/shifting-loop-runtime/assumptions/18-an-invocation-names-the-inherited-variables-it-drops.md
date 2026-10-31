# An invocation names the inherited variables it drops

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`.scratch/shifting-loop-runtime/assumptions/04-an-invocation-carries-its-environment-and-working-directory.md`: "An invocation is a value naming the script, the environment variables added for it and its working directory."

## Assumed
An invocation also names variables of shift's environment that the script doesn't inherit. A variable that is dropped and also added is set to the added value. The loops use this to keep `SHIFT_TASK` and `SHIFT_OUTCOME` unset where the environment table leaves them out.

## Proposed spec text
§8: "An invocation is a value naming the script, the environment variables added for it, the inherited variables it drops, and its working directory."
