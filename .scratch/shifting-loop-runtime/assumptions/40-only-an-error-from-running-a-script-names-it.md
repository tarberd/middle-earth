# Only an error from running a script names it

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`.scratch/shifting-loop-runtime/assumptions/37-a-script-error-names-the-script.md`: "Every error raised while running a loop script names that script's path. This covers a script that exists but can't be executed, and select, commit and rollback failures".

`.scratch/shifting-loop-runtime/assumptions/26-a-failed-git-query-stops-the-run.md`: "If one fails, for example because a script left HEAD unreadable, the run stops with an error naming the git command and git's stderr."

## Assumed
An error names a script when it comes from running that script. That covers starting it, writing its stdin, reading its output and waiting for it to end, and what its exit and stdout mean under the script contract. Reading HEAD when an iteration starts happens before select runs. It is a git query, not part of running select, so its error names the git command (assumption 26) and no script. A count or iteration number that can't grow (assumption 34) is named itself, with no script.

## Proposed spec text
§7 Errors: "Only an error from running a loop script names that script. A failed git query names the git command, and a count that can't grow names the count."
