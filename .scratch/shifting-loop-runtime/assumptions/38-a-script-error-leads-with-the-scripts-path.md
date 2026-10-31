# A script error leads with the script's path

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`.scratch/shifting-loop-runtime/assumptions/37-a-script-error-names-the-script.md`: "Every error raised while running a loop script names that script's path."

`.scratch/shifting-loop-runtime/assumptions/23-loop-errors-carry-the-failure-report.md`: "A select, commit or rollback error names how the script ended and its failure report; commit and rollback errors also name the task."

## Assumed
The error on stderr gives the script's path first, then a colon, then the error as it read before, the way Unix tools name a file in an error. The path is the one the loop directory check found, under the repository root. For example:
- `Error: <root>/.shifting/build/select.sh: select failed (exit status: 1): selector broke`
- `Error: <root>/.shifting/build/commit.sh: commit failed for task t1 (exit status: 1): index.lock exists`
- `Error: <root>/.shifting/build/judge.sh: I/O error: Permission denied (os error 13)`, for a judge that isn't executable.

## Proposed spec text
§7 Errors: "An error raised while running a loop script names the script's path, for example as a `<path>: ` prefix before the error, or within the error's own wording."
