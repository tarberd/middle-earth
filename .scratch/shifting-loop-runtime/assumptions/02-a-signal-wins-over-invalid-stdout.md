# A signal wins over invalid stdout

Filed by: implementer (shifting-loop-runtime 01)

## Spec says
`spec.md` §4: "**Signals.** A step killed by a signal counts as \"anything else\"." and "**Encoding.** Stdout must be valid UTF-8; invalid stdout counts as that step failing." Neither covers both at once.

## Assumed
A script killed by a signal whose stdout is also invalid UTF-8 is reported as killed by that signal. The invalid stdout goes unreported.

## Proposed spec text
§4 Signals: "A step killed by a signal is reported by its signal, even when its stdout is also invalid."
