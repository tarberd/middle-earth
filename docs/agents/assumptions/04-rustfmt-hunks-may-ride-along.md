# Rustfmt hunks may ride along

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`code-review` skill baseline: "**Divergent Change**: one file or module is edited for several unrelated reasons."

## Assumed
A feature commit may carry rustfmt-only hunks in files that weren't fmt-clean, so the tree passes `cargo fmt --check`. Those hunks are not Divergent Change.

## Proposed spec text
`CODING_STANDARDS.md`: "A commit may include rustfmt-only changes that make the files it touches, or the tree, fmt-clean."
