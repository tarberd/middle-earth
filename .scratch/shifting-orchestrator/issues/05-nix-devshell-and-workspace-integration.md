# Nix DevShell and Workspace Integration

Status: ready-for-agent
Blocked by: 04

## Description

Integrate `packages/shifting` into the repository's Nix flake:
- Add `packages/shifting` to the Cargo workspace or package outputs in `flake.nix`.
- Provide a `nix develop .#shifting` devshell exposing compilers and clippy.
- Add mock `.shifting/` templates or integration tests verifying the full flow under Nix.
- Verify `cargo clippy --workspace --all-targets -- -D warnings` across the workspace.

## Acceptance Criteria

- [ ] `nix build .#shifting` or `nix develop .#shifting -c cargo test` succeeds hermetically.
- [ ] Strict Clippy denies pass across all library and binary targets.
- [ ] Ready to be dogfooded by `onehost` or any other package.
