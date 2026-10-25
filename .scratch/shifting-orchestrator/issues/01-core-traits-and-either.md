# Core Category-Theory Traits and Either Coproduct

Status: resolved
Blocked by: None

## Description

Implement the zero-dependency functional core for `shifting`:
- Define `pub enum Either<L, R> { Left(L), Right(R) }` in `shifting::either` with:
  - Consuming collapses to `Option`: `.left(self) -> Option<L>` and `.right(self) -> Option<R>`.
  - Non-consuming reference and deref borrowing: `.as_ref(&self) -> Either<&L, &R>` and `.as_deref(&self) -> Either<&L::Target, &R::Target>`.
  - Converging branch extraction: `Either<T, T>::into_inner(self) -> T`.
  - Bifunctor maps: `.map_left()`, `.map_right()`.
  - Universal catamorphism fold: `.either()`.
  - `Display` and `std::error::Error` implementations when `L` and `R` implement them, allowing `Either<ErrA, ErrB>` to act as a first-class error type.
  - Seamless reciprocal `From`/`Into` conversions with `LoopFlow`.
  - Constructor convenience functions `Either::id()`, `Either::into_left()`, and `Either::into_right()`.
  - Strictly NO `Default` implementation (symmetric sum types have no privileged default).
- Define `pub enum LoopFlow<State, Terminal> { Continue(State), Break(Terminal) }` in `shifting::either` with `.continue_value()`, `.break_value()`, `.as_ref()`, and `.as_deref()`. Strictly NO `Default` implementation.
- Define `pub trait Step<In> { type Out; type Error; fn apply(&self, input: In) -> Result<Self::Out, Self::Error>; }` in `shifting::step`.
- Define `pub struct step::Identity<T>` morphism ($id_A: A \to A$) with `type Error = Infallible`, `const fn new()`, constructor sugar `step::id()`, and standard trait derivations (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Default`).
- Define `pub trait Branch<In> { type Left; type Right; type Error; fn evaluate(&self, input: In) -> Result<Either<Self::Left, Self::Right>, Self::Error>; }` in `shifting::branch`.
- Define `pub struct either::Identity<L, R>` coproduct identity morphism ($id_{L+R}: L+R \to L+R$) with `type Error = Infallible`, implementing both `Step<Either<L, R>>` and `Branch<Either<L, R>>`, with constructor sugars `either::id()` and `Either::id()`, and standard derivations (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`, `Default`).
- Define canonical coproduct injections / value constructors with `type Error = Infallible`, implementing both `Step` and `Branch`, and deriving `Default`:
  - `pub struct either::IntoLeft<L, R>` ($\iota_L: L \to L + R$) with constructor sugars `either::into_left()` and `Either::into_left()`.
  - `pub struct either::IntoRight<L, R>` ($\iota_R: R \to L + R$) with constructor sugars `either::into_right()` and `Either::into_right()`.
- Provide blanket implementations of `Step` and `Branch` for bare closures `F: Fn(...) -> Result<...>` without wrapper structs or method resolution ambiguity.
- Implement combinator traits strictly bounded to pure seams with public attributes (`pub`):
  - Sequential chaining (`StepExt::and_then`): Heterogeneous errors yielding `Result<OutB, Either<ErrA, ErrB>>`. Struct `AndThen<First, Second> { pub first: First, pub second: Second }`.
  - Step-to-Branch chaining (`StepExt::branch`): Heterogeneous errors yielding `Result<Either<L, R>, Either<ErrStep, ErrBranch>>`. Struct `StepBranch<S, B> { pub step: S, pub branch: B }`.
  - Direct cascading branch composition (`BranchExt::branch_both`, `BranchExt::branch_left`, `BranchExt::branch_right`) with structs `BranchBoth`, `BranchLeft`, `BranchRight` having public fields.

## Verification Seams

Public interfaces are strictly tested across three seams:
- **`Step` Seam**: `Step<In>::apply`, `StepExt::and_then`, `StepExt::branch`. Verified for CT laws, heterogeneous error propagation, and bare closure usability.
- **`Branch` Seam**: `Branch<In>::evaluate`, `BranchExt::branch_both`, `BranchExt::branch_left`, `BranchExt::branch_right`. Verified for binary decision tree assembly, short-circuit error propagation, and bare closure usability.
- **`ADT` Seam**: `Either`, `LoopFlow`, canonical injections (`either::IntoLeft`, `either::IntoRight`), coproduct identity (`either::Identity`), projections, catamorphism `.either()`, `into_inner()`, and reciprocal `From`/`Into` conversions.

## Acceptance Criteria

- [x] Zero external dependencies outside `std`.
- [x] Strict functional Rust: zero `&mut`, consumption by value, ADTs.
- [x] Standard trait derivations: `Debug`, `Clone`, `Copy` (conditional), `PartialEq`, `Eq`, `Hash` for `Either` and `LoopFlow` (strictly NO `Default`). `Default` derived on zero-sized identity/injection structs (`step::Identity`, `either::Identity`, `either::IntoLeft`, `either::IntoRight`).
- [x] `Either<L, R>` collapses cleanly to `Option<L>` and `Option<R>` via `.left()` and `.right()`.
- [x] Non-consuming borrowing (`.as_ref()`, `.as_deref()`) on `Either` and `LoopFlow`.
- [x] `Either<T, T>::into_inner()` cleanly extracts the inner value when branches converge.
- [x] `Either<L, R>` implements `Display` and `std::error::Error` when `L` and `R` implement them.
- [x] Canonical injections (`either::IntoLeft`, `either::IntoRight`) and coproduct identity (`either::Identity`) route with zero runtime errors (`Error = Infallible`) and implement both `Step` and `Branch`.
- [x] Canonical `step::Identity<T>` pass-through morphism with `type Error = Infallible`.
- [x] Constructor convenience functions `step::id()`, `either::id()`, `Either::id()`, `either::into_left()`, `either::into_right()`, `Either::into_left()`, and `Either::into_right()`.
- [x] Blanket `Fn` implementations for `Step` and `Branch` work seamlessly with bare closures without wrapper structs or method ambiguity.
- [x] Sequential chaining (`.and_then()`) preserves heterogeneous errors via `Either<ErrA, ErrB>`.
- [x] Step-to-Branch chaining evaluates step output directly into a branch.
- [x] Direct cascading branch composition (`branch_both`, `branch_left`, `branch_right`) functions without intermediate steps.
- [x] Combinator structs (`AndThen`, `StepBranch`, `BranchBoth`, `BranchLeft`, `BranchRight`) expose public fields.
- [x] Unit tests explicitly verify Category Theory identity laws:
  - `step::id().and_then(step) == step`
  - `step.and_then(step::id()) == step`
  - `step.branch(either::id())` preserves coproduct output
  - `branch.branch_both(either::into_left(), either::into_right())` universal coproduct pass-through verification.
- [x] Comprehensive unit tests verifying all three seams, error propagation, and Option/inner collapses.
- [x] Passes `cargo test` and `cargo clippy --all-targets -- -D warnings`.
