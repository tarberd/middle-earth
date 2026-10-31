# Logging stub scripts are not interaction-recording spies

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`CODING_STANDARDS.md` §1: "…avoiding mockable traits and interaction-recording spies."

`.scratch/shifting-loop-runtime/spec.md`, Seam 1: "Stub scripts in `.shifting/plan/`, `.shifting/build/` and `.shifting/review/` append their name, environment and stdin to a log file."

## Assumed
The rule bans in-process test doubles that stand in for a boundary and record the calls made on them. A stub script is a real executable on the far side of the process boundary. Its log records the scripts' observable inputs: which scripts ran, in which order, with which environment and stdin. Those are what the spec's tests check, so such stubs are allowed.

## Proposed spec text
`CODING_STANDARDS.md` §1: "Tests observe behaviour at a real boundary, never through in-process doubles that record the calls made on them. A stub executable that logs what it receives is a real boundary, and tests may assert on its log."
