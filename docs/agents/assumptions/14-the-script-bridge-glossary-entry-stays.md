# The Script Bridge glossary entry stays

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`GLOSSARY.md`: "**Script Bridge**: A protocol adapter in shifting executing executable scripts via Unix STDIN/STDOUT stream piping without filesystem state pollution."

## Assumed
The entry is left as is, although the Script Bridge now also carries per-invocation environment variables, a working directory and a live stderr sink. The invocation isn't added as a glossary term.

## Proposed spec text
`GLOSSARY.md`: "**Script Bridge**: A protocol adapter in shifting executing executable scripts via Unix STDIN/STDOUT stream piping, with per-invocation environment variables and working directory, copying stderr live while capturing it."
