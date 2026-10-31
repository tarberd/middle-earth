# A required script is a file

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §3: "Each command checks that the directories it uses exist and contain every required script, and reports the first missing path, before running anything."

## Assumed
A required script is present when its path is a file. A directory at that path counts as missing. Whether the file is executable isn't checked up front. A script that can't be run stops the run with a Script Bridge error when the loop reaches it. Scripts are checked in the order select, precheck, work, verify, judge, commit, rollback.

## Proposed spec text
§3: "A required script is present when its path is a file. The check doesn't test that it is executable."
