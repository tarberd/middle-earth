# 07: Script Bridge: typed errors for missing pipes and a total output-to-error conversion

Status: needs-triage
Blocked by: None (can start immediately)

## What to build

Rework from the review of assumptions 09, 10 and 12, which updated `CODING_STANDARDS.md` §3.

- **Typed Error Enums** now reads: "Every failure path gets a typed variant, including cases the platform or std rules out … A value that must be present never falls back to a default." Today, when a script's stdin, stdout or stderr handle is missing after being piped, the Script Bridge quietly writes nothing or captures empty output, so a script that exited `0` would look like a successful run with empty output. A missing handle becomes a typed error instead.
- **Parse, Don't Validate** now reads: "A conversion gives every value of its input type a meaningful result … A caller's prior check never stands in for either." Today any script output, a successful one included, converts into the script-failed error, and only the callers' prior check keeps a success from reading as "script failed (exit status: 0)". Make the conversion total, by either approach the standard allows:
  - narrow its input to failed outputs, which only the success/failure split can produce;
  - widen its result so a successful output maps to no error.

The stdin/stdout convention of `ScriptStep` and `ScriptBranch` stays as it is.

## Acceptance criteria

- [ ] A script whose stdin, stdout or stderr handle is missing ends in a typed error, never in empty input or output. No test is needed for it, since only a fake could reach it.
- [ ] A successful script output can't become the script-failed error: the conversion either accepts only failed outputs or gives no error for a successful one.
- [ ] Existing Script Bridge seam tests still pass, adjusted only for the conversion's new shape.
- [ ] Tests and Clippy pass with warnings denied across all targets.
