# A case std rules out that isn't a failure keeps its meaning

Filed by: implementer (shifting-loop-runtime 02)

## Spec says
`CODING_STANDARDS.md` §3: "Every failure path gets a typed variant, including cases the platform or std rules out".

## Assumed
The rule covers failures. A case std rules out that isn't a failure needs no error variant. It is made total by doing what its value means, or avoided by a shape that has no such case. For example, a fold over an endless sequence of passes can only stop at a break, yet its type also lets it hand back a state; handing back a state means the run isn't over, so it runs on from that state:

```rust
match repeat(()).try_fold(state, |state, ()| pass(state)) {
    ControlFlow::Break(ended) => ended,
    ControlFlow::Continue(state) => run(state),
}
```

## Proposed spec text
`CODING_STANDARDS.md` §3: "A case std rules out that isn't a failure is made total by what its value means, or avoided by a shape without it. Only failures need a typed variant."
