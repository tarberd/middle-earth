# Shifting is validated in the onehost devShell

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
`docs/agents/validation.md`: "Run the full test suite for `onehost`: `nix develop .#onehost -c cargo test --manifest-path packages/onehost/Cargo.toml --workspace`". The document names no other crate.

## Assumed
`packages/shifting` is validated in the `onehost` devShell, the only one providing rustc, cargo, clippy and gcc, against its own manifest. It is a single crate, so `--workspace` is dropped:
- `nix develop .#onehost -c cargo test --manifest-path packages/shifting/Cargo.toml`
- `nix develop .#onehost -c cargo clippy --manifest-path packages/shifting/Cargo.toml --all-targets -- -D warnings`

## Proposed spec text
`docs/agents/validation.md`: a `packages/shifting` section giving the two commands above.
