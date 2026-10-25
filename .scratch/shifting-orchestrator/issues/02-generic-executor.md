# Generic State Machine Executor

Status: ready-for-agent
Blocked by: 01

## Description

Implement the zero-dependency, pure categorical state machine execution engine integrated directly into `shifting`'s core (`shifting::executor`):
- Define `pub struct Executor<P> { pub pipeline: P }` in `shifting::executor`, taking any pipeline `P: Step<State, Out = LoopFlow<State, Terminal>>`.
- Constructor sugar: `Executor::new(pipeline)` with standard trait derivations (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`).
- Pure loop execution:
  - `fn run(&self, initial_state: State) -> Result<Terminal, P::Error>`
  - Evaluates transitions iteratively while receiving `LoopFlow::Continue(next_state)`.
  - Halts cleanly and returns `Ok(terminal)` upon receiving `LoopFlow::Break(terminal)`.
  - Error type is strictly `P::Error` with **zero trait bounds** (completely generic to the error type).
  - Iteration limits / safety bounds (e.g. `max_iterations`) are encoded directly into the domain `State` by callers, breaking naturally via `LoopFlow::Break(terminal)` without artificial executor parameters or dual return types.
- Canonical `Step` implementation:
  - Implement `Step<State> for Executor<P>` where `P: Step<State, Out = LoopFlow<State, Terminal>>` with `type Out = Terminal` and `type Error = P::Error`.
  - `Executor<P>` is literally an unconditional morphism $State \to Terminal$ (`Step<State>`).
  - Enables whole state machine loops to be chained into downstream pipelines via `.and_then()` or `.branch()`.
- Combinator sugar on `StepExt`:
  - `fn into_executor(self) -> Executor<Self>` when `Self::Out = LoopFlow<State, Terminal>`.
- Strict functional implementation:
  - Enforce [CODING_STANDARDS.md](../../CODING_STANDARDS.md) §2: No imperative `for` or `while` loops. Use value-producing iterator combinators or pure tail recursion.
  - Zero `&mut`, consumption by value or immutable reference, zero external dependencies outside `core` and `std`.

## Verification Seam

**`Executor` Seam**: Iterative loop convergence, `LoopFlow` continuation/break transitions, domain state bounding, and generic error preservation.
- Catches: Broken state progression, infinite recursion / runaway execution, error propagation defects.
- Misses: Boundary process execution, OS stream piping.

## Acceptance Criteria

- [ ] Zero external dependencies outside `core` and `std`.
- [ ] Error type is strictly `P::Error` with zero trait bounds (no `std::error::Error` or `Display` bounds required on `P::Error`).
- [ ] `Executor<P>` wraps `pipeline: P` with `const fn new(pipeline: P) -> Self` and derives standard traits.
- [ ] `Executor::run(&self, initial_state: State) -> Result<Terminal, P::Error>` executes until `LoopFlow::Break(terminal)`.
- [ ] `impl<State, Terminal, P> Step<State> for Executor<P>` implemented with `type Out = Terminal` and `type Error = P::Error`.
- [ ] `StepExt::into_executor(self)` provides fluent constructor sugar.
- [ ] Pure functional execution without imperative `for` or `while` loops.
- [ ] Unit tests verifying:
  - Single-pass immediate break (`LoopFlow::Break` on first step).
  - Multi-pass loop convergence progressing state across cycles.
  - State-encoded iteration bounding (e.g. `(state, remaining_iterations)`) terminating cleanly via `LoopFlow::Break`.
  - Error short-circuiting returning generic `Err(P::Error)` without wrapping.
  - Composition of `Executor` as a `Step` into a larger pipeline via `.and_then()`.
- [ ] Passes `cargo test` and `cargo clippy --all-targets -- -D warnings`.
