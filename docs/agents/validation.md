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
nix develop .#onehost -c cargo test --manifest-path packages/onehost/Cargo.toml --workspace
```

Run a single integration test during `/tdd` loops:
```bash
nix develop .#onehost -c cargo test --manifest-path packages/onehost/Cargo.toml --test <test_name>
```

## Deterministic Lint Floor (Clippy)

Before declaring any implementation step complete or committing changes, run Clippy across **all workspace crates and targets** (library, binaries, test-support, and tests) with warnings denied:
```bash
nix develop .#onehost -c cargo clippy --manifest-path packages/onehost/Cargo.toml --workspace --all-targets -- -D warnings
```

The build is configured in `packages/onehost/Cargo.toml` and `packages/onehost/clippy.toml` with strict non-negotiable lints across all targets:
- `unused = "deny"`, `dead_code = "deny"`, `unused_variables = "deny"`, `unused_mut = "deny"`, `unused_imports = "deny"`, `unreachable_code = "deny"`, `unused_qualifications = "deny"`: Zero dead code or unused items anywhere in the repository.
- `clippy::allow_attributes = "deny"`: Never suppress lints with `#[allow(...)]`.
- `clippy::used_underscore_binding = "deny"`, `clippy::pub_underscore_fields = "deny"`, `clippy::no_effect_underscore_binding = "deny"`: Never silence unused fields, parameters, or variables with a leading `_`.
- `clippy::unwrap_used = "deny"`, `clippy::panic = "deny"`: No `.unwrap()` or `panic!()` calls in `src/` or `tests/`.
- `clippy::needless_for_each = "deny"`, `clippy::disallowed_methods = "deny"` (`Iterator::for_each`, `Iterator::try_for_each`): Both imperative `for` loops and side-effecting `for_each`/`try_for_each` combinators are forbidden; use value-producing combinators (`map`, `filter`, `fold`, `try_fold`, `collect`).
- `clippy::match_same_arms = "deny"`, `clippy::unnecessary_wraps = "deny"`: Deduplicate match logic and avoid redundant `Result`/`Option` wrappers.

Any warning or error from this command indicates a failed evaluation gate and must be resolved before proceeding to code review.

## Architectural and Standards Validation

For coding standards and design principles:
- Consult [CODING_STANDARDS.md](../../CODING_STANDARDS.md) for the rules evaluated by the `/code-review` skill.
- Consult [ADR-0010](../../docs/adr/0010-functional-rust-idioms.md) for the Pure Core / Imperative Shell architecture rationale and trade-offs.
