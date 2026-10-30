# Bounded polling in tests

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`CODING_STANDARDS.md` §2: "Both imperative `for`/`while` loops and side-effecting `.for_each(...)` / `.try_for_each(...)` combinators are forbidden."

## Assumed
A test that must observe concurrent behaviour may poll with a bounded combinator that sleeps between attempts. Every poll and every stub script has a deadline, so a failure fails the test instead of hanging it:

```rust
(0..500).any(|_| condition() || {
    sleep(Duration::from_millis(20));
    false
})
```

## Proposed spec text
`CODING_STANDARDS.md` §2: "A test that observes concurrent behaviour polls with a bounded combinator and a deadline. No test waits forever."
