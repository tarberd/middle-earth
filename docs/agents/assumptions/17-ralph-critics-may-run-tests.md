# Ralph critics may run tests

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`ralph` skill, step B: each critic gets "`git diff $BASE_SHA...HEAD` and commit log", plus standards files or "the ticket acceptance criteria and spec path/content". Nothing says whether a critic may read other code or run it.

## Assumed
Critics are read-only: they never edit or commit. They may read any file and run the test suite to check a claim.

## Proposed spec text
`ralph` skill, step B: "Critics are read-only. They may read any file and run tests, but never edit or commit."
