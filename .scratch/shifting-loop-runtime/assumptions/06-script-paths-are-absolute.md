# Script paths are absolute

Filed by: implementer (shifting-loop-runtime 01)

## Spec says
Nothing.

## Assumed
Callers pass absolute script paths whenever they set a working directory. The Script Bridge doesn't resolve a relative path, and Rust leaves a relative program path combined with a working directory platform-specific.

## Proposed spec text
§3: "Script paths are resolved against the repository root before any script runs."
