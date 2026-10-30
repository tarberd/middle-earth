# Typed variants for unreachable OS failures

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`CODING_STANDARDS.md` §3: "**Typed Error Enums**: Model domain and boundary failures using explicit `thiserror` enums. Never use stringly-typed errors or bare `anyhow` in domain code."

## Assumed
Failures the platform can't produce on Unix still get their own typed error variants instead of `io::Error::other("…")`. Examples are an exit status with neither a code nor a signal, and a panicked stream thread. The variants are constructed, so they aren't dead code. No test reaches them, since only a fake could.

This conflicts with the assumption that a child's missing pipes, which std rules out after `Stdio::piped()`, take a neutral default. Both cover cases that can't happen, and they treat them in opposite ways.

## Proposed spec text
`CODING_STANDARDS.md` §3: "Boundary failures the platform can't produce still get typed variants. They need no tests when only a fake could reach them."
