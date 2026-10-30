# Std mutable builders at the shell

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`CODING_STANDARDS.md` §2: "**Expression-Oriented**: Prefer expressions and pattern matching (`match`, `if let`) over early returns and procedural state machines."

## Assumed
At the imperative shell, a std builder that mutates through `&mut`, such as `std::process::Command`, may be driven by statements:

```rust
let mut command = Command::new(path);
if let Some(dir) = dir {
    command.current_dir(dir);
}
```

## Proposed spec text
`CODING_STANDARDS.md` §2: "At the imperative shell, std builders that mutate through `&mut`, such as `std::process::Command`, may be driven by statements."
