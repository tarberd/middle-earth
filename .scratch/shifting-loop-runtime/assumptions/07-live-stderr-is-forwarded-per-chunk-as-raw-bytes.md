# Live stderr is forwarded per chunk as raw bytes

Filed by: implementer (shifting-loop-runtime 01)

## Spec says
Ticket 01: "Stderr is forwarded to a live sink as it is written and is also captured in full."

`spec.md` §4: "Stderr is decoded lossily, since it is for humans."

## Assumed
"As it is written" means each chunk of stderr, up to 8 KiB, is forwarded and flushed the moment the pipe yields it. The sink receives stderr's raw bytes. Only the captured copy is decoded lossily.

## Proposed spec text
§8: "Each chunk of stderr is copied to shift's stderr byte for byte as soon as it is read. The captured copy is decoded lossily."
