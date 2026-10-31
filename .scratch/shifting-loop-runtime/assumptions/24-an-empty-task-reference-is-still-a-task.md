# An empty task reference is still a task

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §4: "`0` selected (trimmed stdout becomes `SHIFT_TASK`)". Story 14: "treated as an opaque reference".

## Assumed
Select exiting `0` with blank stdout selects the empty task reference. The iteration runs with `SHIFT_TASK` set to the empty string. The runtime doesn't reject it.

## Proposed spec text
§4: "Select's trimmed stdout is the task reference even when it is empty."
