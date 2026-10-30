# Unreachable missing pipes default to empty

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`CODING_STANDARDS.md` §3: "**Parse, Don't Validate**: Encode invariants directly into algebraic data types (ADTs) at boundaries."

## Assumed
After `Stdio::piped()`, std guarantees that a child's stdin, stdout and stderr handles exist. Code taking them may give a missing handle a neutral default instead of failing: write nothing to stdin, capture empty output.

This conflicts with the assumption that failures the platform can't produce get typed error variants. Both cover cases that can't happen, and they treat them in opposite ways.

## Proposed spec text
`CODING_STANDARDS.md` §3: "An `Option` that std guarantees is `Some`, such as a child's pipe after `Stdio::piped()`, may take a neutral default instead of a typed error."
