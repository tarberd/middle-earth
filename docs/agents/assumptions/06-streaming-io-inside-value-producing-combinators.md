# Streaming I/O inside value-producing combinators

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`CODING_STANDARDS.md` §2: "Transform collections and sequences using value-producing iterator combinators (`map`, `filter`, `fold`, `try_fold`, `chain`, `collect::<Result<_, _>>()`) across both `src/` and `tests/`. Both imperative `for`/`while` loops and side-effecting `.for_each(...)` / `.try_for_each(...)` combinators are forbidden."

## Assumed
At the imperative shell, a value-producing combinator may perform I/O while producing its value. Copying a stream chunk by chunk to a writer inside `.map` and collecting the chunks is allowed. Since loops are banned, this is how streaming I/O is written:

```rust
repeat_with(|| read_chunk(&mut source))
    .take_while(|chunk| !is_eof(chunk))
    .map(|chunk| chunk.and_then(|bytes| sink.write_all(&bytes).map(|()| bytes)))
    .collect::<io::Result<Vec<_>>>()
```

## Proposed spec text
`CODING_STANDARDS.md` §2: "At the imperative shell, streaming I/O is written as a value-producing combinator chain that performs the I/O inside `map` and collects the results."
