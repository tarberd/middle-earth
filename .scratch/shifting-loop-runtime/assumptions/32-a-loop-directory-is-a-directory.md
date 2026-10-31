# A loop directory is a directory

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §3: "Each command checks that the directories it uses exist and contain every required script, and reports the first missing path, before running anything."

## Assumed
A loop directory is present when its path, followed through symbolic links, is a directory. A file or anything else at that path is reported as the missing loop directory. A required script likewise counts as what its symbolic link points to (`.scratch/shifting-loop-runtime/assumptions/25-a-required-script-is-a-file.md`).

## Proposed spec text
§3: "A loop directory is present when its path, followed through symbolic links, is a directory. Anything else at that path is reported as missing."
