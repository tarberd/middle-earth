# Arithmetic overflow is a failure path

Filed by: orchestrator (shifting-loop-runtime 02)

## Spec says
`CODING_STANDARDS.md` §3: "Every failure path gets a typed variant, including cases the platform or std rules out" and "Neither `src/` nor `tests/` may call `.unwrap()` or `panic!()`."

## Assumed
Production integer arithmetic that can overflow is a failure path, even when no feasible run reaches the bound: it panics in a debug build and wraps silently in a release build. It never panics or wraps; overflow becomes a typed error. Either design satisfies this, for example:
- checked arithmetic that maps overflow to a typed error;
- a counter type whose increment returns a typed error.

Arithmetic whose operand types already rule out overflow needs no check.

## Proposed spec text
`CODING_STANDARDS.md` §3: "Integer arithmetic that can overflow never panics or wraps: overflow is a typed error, for example through checked arithmetic or a counter type whose increment can fail. Arithmetic whose operand types rule out overflow needs no check."
