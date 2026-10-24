# Generic State Machine Executor

Status: ready-for-agent
Blocked by: 01, 02

## Description

Implement the generic execution engine that drives state transitions:
- Define `Executor`: takes an initial state and a composed pipeline producing `LoopFlow<State, Terminal>`.
- Executes transitions iteratively while `LoopFlow::Continue(next_state)` is received.
- Halts cleanly and returns `Terminal` when `LoopFlow::Break(terminal)` is received.
- Supports an optional `max_iterations` safety bound to prevent runaway loops.

## Acceptance Criteria

- [ ] Stateless, pure loop execution without mutable internal state.
- [ ] Safe termination when hitting `max_iterations` or `LoopFlow::Break`.
- [ ] Unit tests testing single-pass execution, multi-pass loop convergence, and max iteration cutoff.
- [ ] Passes `cargo test` and `cargo clippy --all-targets -- -D warnings`.
