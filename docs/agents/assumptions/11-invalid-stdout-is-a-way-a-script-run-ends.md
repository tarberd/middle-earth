# Invalid stdout is a way a script run ends

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`code-review` skill baseline: "**Mysterious Name**: a function, variable, or type whose name doesn't reveal what it does or holds."

## Assumed
The Script Bridge's account of how a script run ended has three cases: an exit code, a kill by signal, or invalid stdout together with its exit code. From the bridge's point of view, a run with invalid stdout ended as a failure. So the exit type keeps its name and holds this case, rather than invalid stdout becoming a separate decoding flag.

## Proposed spec text
`GLOSSARY.md`: "**Script Exit**: How the Script Bridge classifies the end of a script run: an exit code, a kill by signal, or invalid stdout with its exit code."
