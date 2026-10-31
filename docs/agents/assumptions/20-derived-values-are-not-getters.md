# Derived values are not getters

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`CODING_STANDARDS.md` §4: "**No Setters and Getters**: Treat all objects as immutable with public attributes, leveraging move semantics for setting new values."

## Assumed
The rule forbids methods that only return or replace a field. A method that computes a value from the public fields, such as a total of counts or the name a variant goes by, isn't a getter. It is preferred over a stored copy of the value, which could disagree with the fields it derives from.

```rust
pub const fn iterations(&self) -> u64 {
    self.commits + self.already_done + self.declines
}
```

## Proposed spec text
`CODING_STANDARDS.md` §4: "A method that computes a value from public fields isn't a getter; prefer it to storing a derived copy."
