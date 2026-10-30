# Rustfmt is part of the validation gate

Filed by: orchestrator (shifting-loop-runtime 01)

## Spec says
Nothing. `docs/agents/validation.md` lists tests and Clippy only.

## Assumed
`cargo fmt --check` also passes before every commit, run in the same devShell against the crate's manifest.

## Proposed spec text
`docs/agents/validation.md`: "Formatting: `nix develop .#onehost -c cargo fmt --manifest-path <crate>/Cargo.toml --check` passes before every commit."
