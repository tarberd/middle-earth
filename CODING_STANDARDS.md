# Coding Standards

Coding standards and architectural rules for this repository. These standards are enforced during development and evaluated by the `code-review` skill.

## Rust (`packages/onehost`)

See [ADR-0010 (Functional Rust Idioms and Pure Core Architecture)](docs/adr/0010-functional-rust-idioms.md).

### 1. Pure Core / Imperative Shell
- **Stateless Domain Logic**: Planners, parsers, and XML AST transformations must be pure, deterministic functions that take immutable inputs and return plain data structures.
- **Isolate Side Effects**: All side effects must sit at the boundary: hypervisor interactions sit behind the `Hypervisor` boundary port (implemented by `VirshHypervisor` in production and an in-memory `FakeHypervisor` in tests), while disk and image manipulations directly utilize the local-substitutable `QemuImgStorage` toolchain against temporary directories, avoiding mockable traits and interaction-recording spies.
- **Canonical Exemplar**: See [`packages/onehost/src/lifecycle/planner.rs`](packages/onehost/src/lifecycle/planner.rs) (`DomainLifecyclePlanner`) and [`packages/onehost/src/domain/element.rs`](packages/onehost/src/domain/element.rs) (`DomainXmlElement`).

### 2. Immutability & Functional Combinators
- **Expression-Oriented**: Prefer expressions and pattern matching (`match`, `if let`) over early returns and procedural state machines.
- **Combinators over Loops**: Transform collections and sequences using value-producing iterator combinators (`map`, `filter`, `fold`, `try_fold`, `chain`, `collect::<Result<_, _>>()`) across both `src/` and `tests/`. Both imperative `for`/`while` loops and side-effecting `.for_each(...)` / `.try_for_each(...)` combinators are forbidden.
- **Functional Builders**: When building or mutating domain structures, use functional builder patterns (`fn with_*(self, ...) -> Self`) returning new or updated instances.

### 3. Total Error Handling & Explicit Types
- **No Unchecked Unwraps or Panics**: Neither `src/` nor `tests/` may call `.unwrap()` or `panic!()`. Use `?` in production code and `.expect("reason")` or `assert!(matches!(...))` in tests.
- **Typed Error Enums**: Model domain and boundary failures using explicit `thiserror` enums. Never use stringly-typed errors or bare `anyhow` in domain code. Every failure path gets a typed variant, including cases the platform or std rules out; such variants need no tests when only a fake could reach them. A value that must be present never falls back to a default.
- **Parse, Don't Validate**: Encode invariants directly into algebraic data types (ADTs) at boundaries. A conversion gives every value of its input type a meaningful result. When only some inputs have one, either narrow the input to a type holding just those values (output → failed output → error) or widen the output to represent the missing results (output → `Option<error>`); both are fine. A caller's prior check never stands in for either.

### 4. Algebraic Data Types & Zero Dead Code
- **Zero Mutation**: Objects must be born in a valid state. All ADT methods must consume `self` or `&self`. No `&mut self` is allowed anywhere.
- **No Setters and Getters**: Treat all objects as immutable with public attributes, leveraging move semantics for setting new values.
- **Zero Dead Code**: Unused struct fields, function/method parameters, types, and local bindings must never exist anywhere in the repository (`src/` or `tests/`) for any reason. Delete unused fields, parameters, and bindings outright — never silence compiler warnings with a leading underscore (`_field`, `_param`, `_var`) or `#[allow(...)]` / `#[expect(...)]` attributes.

