# A task reference over several lines is shown escaped

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`.scratch/shifting-loop-runtime/assumptions/28-a-task-reference-is-one-line.md`: "When select exits `0` and its trimmed stdout still contains a line break (`\n` or `\r`), the run stops with a typed select error showing that stdout".

## Assumed
The error shows select's trimmed stdout quoted on one line, with its line breaks escaped, for example `select chose a task reference that spans several lines: "line one\nline two"`. The error stays one line on stderr, and every `\n` and `\r` stays visible.

## Proposed spec text
§7 Errors: "A select error for a task reference over several lines shows the trimmed stdout so that each line break is visible, for example quoted with its line breaks escaped."
