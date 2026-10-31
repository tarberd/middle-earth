# A loop path that can't be inspected is an error, not missing

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`spec.md` §3: "Each command checks that the directories it uses exist and contain every required script, and reports the first missing path, before running anything."

`CODING_STANDARDS.md` §3: "Every failure path gets a typed variant".

## Assumed
A loop directory or script counts as missing only when the file system reports that it doesn't exist, which includes a dangling symbolic link. When a path can't be inspected for another reason, such as a permission error, the check stops with a typed error naming the path and its cause. That still happens before any script runs, and the path is never reported as missing.

## Proposed spec text
§3: "…and reports the first missing path, before running anything. A path that can't be inspected is reported with its cause, not as missing."
