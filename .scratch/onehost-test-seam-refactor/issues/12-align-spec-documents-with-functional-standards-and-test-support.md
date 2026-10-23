# 12: Align Spec Documents with Functional Coding Standards and Test Support Layout

**What to build:** Updates the test-seam refactoring spec and the declarative reconciliation spec so that documented architectural decisions, test-support crate paths, and functional Rust coding standards match the implemented codebase.

**Blocked by:** 11: Eliminate Storage Middle-Man Delegation and Deduplicate Destroyer Tests

**Status:** ready-for-agent

- [ ] The test-seam refactoring spec reflects the pure `snapshot()` inspection model on `FakeHypervisor`, removal of unused `ApplyOptions`, expression-oriented scratch cleanup in `ImageBuilder`, and functional Rust lint enforcement.
- [ ] The declarative reconciliation spec references the `onehost-test-support` workspace crate rather than the outdated `tests/common/fake_hypervisor.rs` path.
