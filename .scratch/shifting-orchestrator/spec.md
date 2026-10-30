# Shifting: Category-Theory Type-State Machine for Agentic Orchestration

Status: ready-for-agent

> **Superseded in part (2026-10-30):** the five-script default loop convention, user stories 11 and 12, and the Architecture & State Topology diagram are replaced by [`shifting-loop-runtime`](../shifting-loop-runtime/spec.md) and [`shifting-default-loop`](../shifting-default-loop/spec.md) (ADR-0011 as amended). The category-theory core described here is unchanged.

## Problem Statement

Autonomous agentic software engineering loops frequently suffer from two fatal failure modes: **context rot / token explosion** when orchestrating multi-agent loops in long-running single conversations, and **premature victory / false self-reporting** when trusting the agent's internal claim that code passes tests or adheres to invariants. Existing generic wrappers (like `ralph-loop` or `ralph-harness`) are either tightly coupled to specific language runtimes (Bun, TypeScript), hardcoded to single vendor CLIs (`claude -p`), unaware of hermetic devshells (Nix), or reliant on brittle waterfall YAML task representations that break on the first unexpected refactor.

## Solution

A standalone, zero-dependency Type-State machine and orchestrator crate named `shifting` in `packages/shifting`. The solution:
- Defines an unopinionated, mathematically sound Category Theory core:
  - **Category**: Objects are Rust types, and morphisms (arrows) are transitions represented by `Step<In, Out>` returning `Result<Out, Error>`.
  - **Arrow Composition**: Chained via `.and_then()` satisfying categorical associativity ($(f \circ g) \circ h = f \circ (g \circ h)$) and identity laws ($id_B \circ f = f$ and $f \circ id_A = f$).
  - **Step Identity Morphism**: `step::Identity<T>` ($id_A: A \to A$) with `type Error = Infallible`, `const fn new()`, constructor sugar `step::id()`, and standard trait derivations.
  - **Coproducts (Sum Types)**: `Either<L, R>` representing the categorical direct sum ($L + R$).
  - **Canonical Injections / Value Constructors**: `either::IntoLeft<L, R>` ($\iota_L: L \to L + R$) and `either::IntoRight<L, R>` ($\iota_R: R \to L + R$) with `type Error = Infallible`, implementing both `Step` and `Branch`, instantiated via `either::into_left()`, `either::into_right()`, `Either::into_left()`, `Either::into_right()`.
  - **Coproduct Identity**: `either::Identity<L, R>` ($id_{L+R}: L+R \to L+R$) with `type Error = Infallible`, implementing both `Step` and `Branch`, instantiated via `either::id()` and `Either::id()`.
  - **Universal Eliminator / Catamorphism**: `.either(fl, fr) -> C` mapping $L + R \to C$ given morphisms $fl: L \to C$ and $fr: R \to C$.
  - **Decision Morphisms**: `Branch<In, Left, Right>` mapping $A \to L + R$ as a pure decision coproduct morphism.
  - **Iterative Execution**: Driven by control-flow sum types (`LoopFlow<State, Terminal>`).
- Supports **heterogeneous error preservation** across sequential composition (`Either<ErrA, ErrB>`), step-to-branch handoff (`Either<ErrStep, ErrBranch>`), and cascading decision trees without forcing artificial common error types or dynamic trait objects.
- Enables **direct branch-to-branch cascading** (`branch_both`, `branch_left`, `branch_right`) allowing decision trees to branch directly on both arms into sub-branches without dummy intermediate steps.
- Equips `Either<L, R>` and `LoopFlow<S, T>` with ergonomic collapses to `Option` (`.left()`, `.right()`, `.continue_value()`, `.break_value()`), non-consuming borrowing (`.as_ref()`, `.as_deref()`), converging extraction (`Either<T, T>::into_inner()`), and bifunctor maps (`.map_left()`, `.map_right()`).
- Implements a Unix convention bridge (`ScriptStep`, `ScriptBranch`) transferring state payloads as raw UTF-8 strings across standard STDIN/STDOUT streams, keeping application logic decoupled and language-agnostic.
- Ships with a canonical 5-script default loop convention in `.shifting/`:
  - `plan.sh` (`Step`): Selects next work unit (exit `0` = work planned, exit `2` = backlog empty / break).
  - `work.sh` (`Step`): Executes agent worker in fresh out-of-process CLI context.
  - `verify.sh` (`Branch`): Runs compiler, linters, and test suites (exit `0` = pass, non-zero = fail with stderr).
  - `commit.sh` (`Step`): Stages changes and creates an atomic git commit on pass.
  - `rollback.sh` (`Step`): Reverts working tree (`git reset --hard`) on fail.
  - Enforces strict functional Rust invariants ([ADR-0010](../../docs/adr/0010-functional-rust-idioms.md)): pure core, immutability, zero `&mut`, consumption by value, and total error handling without `unwrap()` or `panic!()`.

## Verification Seams

The library's public boundary is strictly partitioned into three verifiable seams:
- **`Step` Seam**: Unconditional transformation morphisms (`Step<In>::apply`), sequential chaining (`StepExt::and_then`), and step-to-branch transitions (`StepExt::branch`). Catches: broken Category Theory identity/associativity laws, sequential error propagation (`Either<ErrA, ErrB>`), and bare closure ergonomics. Misses: branching decisions.
- **`Branch` Seam**: Pure conditional decision coproduct morphisms (`Branch<In>::evaluate`), and cascading binary decision trees (`BranchExt::branch_both`, `BranchExt::branch_left`, `BranchExt::branch_right`). Catches: decision routing, tree assembly, and short-circuit error propagation. Misses: unconditional state transformations.
- **`ADT` Seam**: Coproduct (`Either`) and control-flow (`LoopFlow`) sum types, canonical injections (`either::IntoLeft`, `either::IntoRight`), coproduct identity (`either::Identity`), projections (`.left()`, `.right()`, `.continue_value()`, `.break_value()`), catamorphism (`.either()`), converging extraction (`Either<T, T>::into_inner()`), and reciprocal `From`/`Into` conversions. Catches: algebraic invariant violations and projection defects. Misses: execution pipeline composition.

## User Stories

1. As a systems developer, I want to compose an agentic state machine statically in Rust using pure Category Theory morphisms, so that illegal state transitions are rejected at compile time.
2. As an agent orchestrator author, I want transitions modeled as composable morphisms (`Step`), coproduct branches (`Branch`), canonical injections (`either::IntoLeft`, `either::IntoRight`), and coproduct identity (`either::Identity`), so that arbitrary execution graphs can be assembled cleanly.
3. As a developer, I want `Step` composition to strictly satisfy Category Theory identity and associativity laws, so that pipelines behave with mathematical predictability.
4. As a developer, I want pure decision branching with heterogeneous error propagation across sequential transitions (`Either<ErrA, ErrB>`), step-to-branch handoff (`Either<ErrStep, ErrBranch>`), and decision cascades (`Either<ErrBranch, Either<ErrL, ErrR>>`), so that divergent paths do not require boxed trait objects or common wrapper enums.
5. As an orchestrator author, I want branches to be able to branch directly into other branches via `.branch_both()`, `.branch_left()`, and `.branch_right()` without intermediate no-op steps, so that complex decision trees remain concise.
6. As a developer, I want `Either<L, R>` to collapse into `Option` via `.left()` and `.right()` and provide `.map_left()`, `.map_right()`, and `.either()`, so that consuming outcomes is idiomatic.
7. As a developer, I want `step::id()`, `either::id()`, `either::into_left()`, and `either::into_right()` morphisms, so that pass-through pipeline and branch arms require zero boilerplate.
8. As a developer, I want zero third-party dependencies in `shifting`'s core crate, so that it compiles instantaneously and adds zero dependency churn to workspaces.
9. As an operator running agent loops, I want every iteration to run out-of-process with fresh context, so that token consumption remains bounded and context rot is eliminated.
10. As an operator, I want all state communication between the runner and external scripts to flow through standard Unix STDIN and STDOUT, so that steps can be implemented in Bash, Python, or Rust without SDK dependencies.
11. As a developer, I want `plan.sh` to signal loop completion via exit code `2`, so that the loop terminates deterministically without brittle string scraping.
12. As a systems administrator, I want non-zero exits from `verify.sh` to automatically trigger `rollback.sh`, so that unverified or broken code can never land in git.
13. As a developer, I want to construct a custom orchestrator loop in under 20 lines of Rust in `main.rs`, so that project-specific verification gates (like Nix DevShells or Clippy) can be injected cleanly.

## Architecture & State Topology

```
                  ┌────────────────────────┐
                  │      State: Idle       │
                  └───────────┬────────────┘
                              │ ScriptStep("plan.sh")
                              ▼
                  ┌────────────────────────┐
                  │     State: Planned     │
                  └───────────┬────────────┘
                              │ ScriptStep("work.sh")
                              ▼
                  ┌────────────────────────┐
                  │    State: WorkDone     │
                  └───────────┬────────────┘
                              │ ScriptBranch("verify.sh")
                   ┌──────────┴──────────┐
      Either::Left │                     │ Either::Right
      (Exit 0)     ▼                     ▼ (Exit != 0)
         ┌───────────────────┐ ┌───────────────────┐
         │  State: Verified  │ │  State: Rejected  │
         └─────────┬─────────┘ └─────────┬─────────┘
                   │ commit.sh           │ rollback.sh
                   ▼                     ▼
         ┌───────────────────┐ ┌───────────────────┐
         │     Committed     │ │    RolledBack     │
         └─────────┬─────────┘ └─────────┬─────────┘
                   └──────────┬──────────┘
                              ▼
                   LoopFlow::Continue(Idle)
```
