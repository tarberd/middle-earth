# Verified carries the work summary

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §6: "`Verified{task, base, output}`". `spec.md` §4: "`judge.sh` | work summary". Ticket 02: "Verify runs only after work succeeds, and judge only after verify passes. Each receives the work summary."

## Assumed
`Verified` carries the work summary, which judge receives on stdin, instead of verify's stdout. Nothing after verify reads verify's stdout, so the state doesn't keep it.

## Proposed spec text
§6: "`Verified{task, base, summary}`, where `summary` is the work summary that judge receives."
