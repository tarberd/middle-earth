# Coding Standards

Coding standards and architectural rules for this repository. These standards are enforced during development and evaluated by the `code-review` skill.

## Rust (`packages/onehost`)

See [ADR-0010 (Functional Rust Idioms and Pure Core Architecture)](docs/adr/0010-functional-rust-idioms.md).

### 1. Pure Core / Imperative Shell
- **Stateless Domain Logic**: Planners, parsers, and XML AST transformations must be pure, deterministic functions that take immutable inputs and return plain data structures.
- **Isolate Side Effects**: All I/O, hypervisor calls, storage manipulations, and child processes must sit behind mockable traits (`Hypervisor`, `StorageManager`, `ProcessRunner`) in the lifecycle applier or CLI boundary.
- **Canonical Exemplar**: See [`packages/onehost/src/lifecycle/planner.rs`](packages/onehost/src/lifecycle/planner.rs) (`DomainLifecyclePlanner`) and [`packages/onehost/src/domain/element.rs`](packages/onehost/src/domain/element.rs) (`DomainXmlElement`).

### 2. Immutability & Functional Combinators
- **Expression-Oriented**: Prefer expressions and pattern matching (`match`, `if let`) over early returns and procedural state machines.
- **Combinators over Loops**: Transform collections and sequences using iterator combinators (`map`, `filter`, `fold`, `try_fold`, `chain`, `collect::<Result<_, _>>()`) instead of imperative `for`/`while` loops with mutable accumulators.
- **Functional Builders**: When building or mutating domain structures, use functional builder patterns (`fn with_*(self, ...) -> Self`) returning new or updated instances.

### 3. Total Error Handling & Explicit Types
- **No Unchecked Unwraps or Panics**: Production code in `src/` must never call `.unwrap()`, `.expect()`, or `panic!()`.
- **Typed Error Enums**: Model domain and boundary failures using explicit `thiserror` enums. Never use stringly-typed errors or bare `anyhow` in domain code.
- **Parse, Don't Validate**: Encode invariants directly into algebraic data types (ADTs) at boundaries.

### 4. Algebraic data types over Object Oriented Programmin
- **Zero mutation**: Objects should be born already in valid state. All ADT methods must consume `self` or `&self`. No `&mut self` allowed anywhere.
- **No seters and getters**: All objects should be treated as immutable, with public attributes, and leverage move semantics for setting new values.

