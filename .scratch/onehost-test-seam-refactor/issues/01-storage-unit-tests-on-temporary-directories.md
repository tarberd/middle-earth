# 01: Storage Unit Tests on Real Toolchain in Temporary Directories

**What to build:** Migrates storage unit testing to exercise real QEMU disk operations against isolated temporary directories rather than in-memory storage record simulations. Tests validate physical overlay allocation, image inspection, thin backup conversion, and safe file moves directly using host filesystem and image utilities.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] Storage unit tests operate against ephemeral temporary directories using real disk image tools.
- [x] In-memory mock storage records are eliminated from storage unit test assertions.
- [x] All storage unit tests pass hermetically in the Nix devShell.
