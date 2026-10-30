# An invocation carries its environment and working directory

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`spec.md` §8: "The Script Bridge gains environment variables and a working directory per invocation."

## Assumed
An invocation is a value naming the script, the environment variables added for it and its working directory. It is built first and then run with its stdin. The environment and working directory aren't passed separately at run time. A caller whose variables change every iteration, such as `SHIFT_TASK`, builds a new invocation each iteration.

## Proposed spec text
§8: "The Script Bridge gains environment variables and a working directory per invocation. An invocation is a value naming the script, its added environment variables and its working directory."
