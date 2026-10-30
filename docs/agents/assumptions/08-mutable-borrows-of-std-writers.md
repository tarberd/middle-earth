# Mutable borrows of std writers

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`CODING_STANDARDS.md` §4: "**Zero Mutation**: Objects must be born in a valid state. All ADT methods must consume `self` or `&self`. No `&mut self` is allowed anywhere."

## Assumed
The rule covers the receivers of the repository's own methods. A parameter may borrow a std reader or writer mutably:

```rust
fn run(&self, input: &str, sink: &mut impl Write) -> Result<Output, Error>
```

## Proposed spec text
`CODING_STANDARDS.md` §4: "No `&mut self` on the repository's own types. A parameter may borrow a std reader or writer as `&mut`."
