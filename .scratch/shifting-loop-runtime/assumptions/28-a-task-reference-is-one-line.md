# A task reference is one line

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`spec.md` §4: "`0` selected (trimmed stdout becomes `SHIFT_TASK`)". §7: "The last stdout line names the end state." Story 14: "treated as an opaque reference".

## Assumed
A task reference is one line. When select exits `0` and its trimmed stdout still contains a line break (`\n` or `\r`), the run stops with a typed select error showing that stdout, and no other script runs in that iteration. Otherwise a `Declined{task}` end state would span several lines, and the single line on stdin of `.scratch/shifting-loop-runtime/assumptions/16-the-task-reference-arrives-as-a-line.md` wouldn't hold. Within one line, the reference stays opaque, and an empty one is still a task (`.scratch/shifting-loop-runtime/assumptions/24-an-empty-task-reference-is-still-a-task.md`).

## Proposed spec text
§4, `select.sh`: "`0` selected (trimmed stdout becomes `SHIFT_TASK`; a trimmed stdout containing a line break stops the run with an error)".
