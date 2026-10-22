# Validation and Testing Guide

How agents validate, test, and verify code changes in this repository.

## Environment: Hermetic Nix DevShell

This repository is managed with Nix. Compilers, linkers, and hypervisor utilities are hermetically packaged in the `onehost` devShell.

Never run `cargo` directly without entering the devShell, as system linkers (`cc`) and QEMU test utilities are not exposed in global `$PATH`.

Invoke all Rust commands through:
```bash
nix develop .#onehost -c <command>
```

## Running Tests

Run the full test suite for `onehost`:
```bash
nix develop .#onehost -c cargo test --manifest-path packages/onehost/Cargo.toml
```

Run a single integration test during `/tdd` loops:
```bash
nix develop .#onehost -c cargo test --manifest-path packages/onehost/Cargo.toml --test <test_name>
```

## Deterministic Lint Floor (Clippy)

Before declaring any implementation step complete or committing changes, run Clippy against the library and binaries:
```bash
nix develop .#onehost -c cargo clippy --manifest-path packages/onehost/Cargo.toml --lib --bins
```

The build is configured in `packages/onehost/Cargo.toml` with strict non-negotiable lints:
- `clippy::unwrap_used = "deny"`: No `.unwrap()` calls in production code.
- `clippy::panic = "deny"`: No unhandled `panic!()` invocations in production code.
- `clippy::needless_for_each = "warn"`: Prefer idiomatic iterator loops or combinators.
- `clippy::match_same_arms = "warn"`: Deduplicate match logic.

Any warning or error from this command indicates a failed evaluation gate and must be resolved before proceeding to code review.

## Architectural and Standards Validation

For coding standards and design principles:
- Consult [CODING_STANDARDS.md](../../CODING_STANDARDS.md) for the rules evaluated by the `/code-review` skill.
- Consult [ADR-0010](../../docs/adr/0010-functional-rust-idioms.md) for the Pure Core / Imperative Shell architecture rationale and trade-offs.
