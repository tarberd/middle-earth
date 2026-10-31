# The loop report format

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`spec.md` §7: "The last stdout line names the end state." Ticket 02: "prints its loop summary and then its end state as the last stdout line".

## Assumed
On a clean end, stdout is one `name: count` line per summary count, in this order: `iterations`, `commits`, `already-done`, `work-failed`, `verify-failed`, `judge-failed`, `declines`. The last line is the end state: `Exhausted`, `MaxIterations`, or `Declined <task>`. When the run stops with an error, nothing is printed on stdout.

## Proposed spec text
§7: "On a clean end, shift prints the summary as `name: count` lines, then the end state (`Exhausted`, `MaxIterations` or `Declined <task>`) as the last line. An error prints nothing on stdout."
