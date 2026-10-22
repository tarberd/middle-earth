# Onehost Test Seam Refactoring and Mock Retirement

Status: ready-for-agent

## Problem Statement

Developers and coding agents working on `onehost` must maintain over 1,200 lines of synthetic mock machinery across production modules (`MockHypervisor`, `MockStorageManager`, `MockProcessRunner`). Existing lifecycle and storage tests assert heavily on brittle, internal call-recording spies (`RecordedStorageAction`, `RecordedHypervisorAction`), causing test suites to break during refactoring even when external behavior is preserved. Furthermore, `StorageManager` and `ProcessRunner` are single-adapter traits that inject unnecessary generic parameters (`<H, S, P>`) across core structs in the codebase, introducing unwarranted indirection and boilerplate. Storage operations are mocked in-memory despite real `qemu-img` toolchains and filesystem operations being fast, hermetic, and local-substitutable in the Nix environment.

## Solution

Refactor `onehost`'s test seams in accordance with the Pure Core / Imperative Shell architecture and Matt Pocock's seam discipline:
- Retire `MockStorageManager` and collapse `trait StorageManager` directly into the concrete `QemuImgStorage` manager used across production and tests (in ephemeral temporary workspaces).
- Purge all call-recording spies (`RecordedHypervisorAction`, `RecordedStorageAction`, `RecordedProcessAction`), updating tests to assert strictly on observable domain and disk states.
- Relocate `FakeHypervisor` out of production crate sources into shared test fixtures, retaining `trait Hypervisor` as the sole external system boundary port.
- Retire `MockProcessRunner` and `trait ProcessRunner`, simplifying image building to use direct process execution.

## User Stories

1. As a developer writing new lifecycle logic, I want tests to assert on observable domain and disk state rather than internal call traces, so that internal refactorings do not break passing tests.
2. As a developer extending the hypervisor abstraction, I want test doubles located in test-only directories, so that production library code remains free of testing fixtures.
3. As a developer modifying instance storage operations, I want storage tests to run against real disk images via `QemuImgStorage` in temporary directories, so that tests validate actual toolchain behavior rather than simulated in-memory models.
4. As a developer maintaining the `onehost` codebase, I want `QemuImgStorage` to be a concrete type rather than a generic trait, so that structs like the lifecycle applier, destroyer, planner, and backup engine do not carry unnecessary generic type parameters.
5. As an agent implementing infrastructure features, I want the `Hypervisor` trait to be the single boundary port in the codebase, so that architectural boundaries are clear and focused.
6. As a developer testing the lifecycle applier, I want `FakeHypervisor` to model observable virtual machine states (`Running`, `ShutOff`, defined XML) in memory, so that lifecycle transitions can be verified without calling live hypervisors.
7. As an operator running `onehost` on Gandalf, I want automated test suites to execute without invoking live `virsh` commands, so that the running production Windows instance is never endangered or disrupted by tests.
8. As a developer inspecting compiler errors, I want all production code to omit testing structs (`MockHypervisor`, `MockStorageManager`, `MockProcessRunner`), so that auto-complete and compiler suggestions only expose production APIs.
9. As a developer writing disaster recovery tests, I want backup and restore engines to use real `qemu-img` thin backup compression and rebase operations, so that disaster recovery guarantees hold against real image formats.
10. As a developer testing image building, I want the image builder to execute child processes directly without a synthetic process runner mock, so that build mechanics remain simple and predictable.
11. As a developer working with `onehost` domain diffing and planning, I want the planner to operate on the hypervisor port and concrete storage manager without secondary mock layers, so that planning verification remains fast and direct.
12. As a developer refactoring domain deprovisioning, I want tests to verify that overlay disk files are physically unlinked from temporary storage, so that file cleanup is verified through the filesystem rather than recorded action logs.
13. As a developer adding new hypervisor capabilities, I want to implement methods only on `VirshHypervisor` and `FakeHypervisor`, so that I do not have to write call-recording enum variants for every new operation.
14. As an automated test runner in a hermetic Nix sandbox, I want the entire test suite to pass without requiring root privileges or a running Libvirt daemon, so that builds and CI runs remain deterministic.

## Implementation Decisions

### 1. Storage Seam Consolidation
- Collapse the storage trait into the concrete QEMU disk image storage manager.
- Remove the generic storage type parameter (`S: StorageManager`) across all domain lifecycle structs (applier, destroyer, planner), backup engines (backup engine, restore engine), and CLI entry points.
- Delete the in-memory mock storage manager from production sources.
- Direct all storage unit and integration test fixtures to execute against temporary directories created via `tempfile::tempdir()`.

### 2. Hypervisor Test Double Relocation and State-Based Simplification
- Relocate the in-memory hypervisor test double from production crate sources into a shared test-only fixture module (`tests/common/fake_hypervisor.rs`).
- Purge all call-recording action enums and recorded action vectors.
- Maintain domain state as an in-memory mapping of domain names to their running state, XML definitions, block device mappings, and pool paths.
- Provide state inspection helper methods for tests (e.g. querying defined domains or checking active states) through public test-only methods on the fake.

### 3. Elimination of Call-Spy Assertions Across Test Suites
- Update lifecycle, planner, and disaster recovery tests to replace assertions on recorded storage and hypervisor action vectors with state assertions.
- Verify storage operations by inspecting physical files in temporary directories (e.g., verifying that overlay disks exist, backing files point to the expected base images via disk inspection, or old overlays are unlinked).
- Verify hypervisor operations by querying domain registration and state on the fake hypervisor.

### 4. Process Execution Simplification
- Remove the generic process runner trait and its associated mock implementation and recorded process action enum.
- Simplify the image builder to execute child processes directly using standard system process utilities.

### 5. Architectural Boundary Preservation
- Maintain `trait Hypervisor` as the sole external system boundary port in the codebase, implemented by `VirshHypervisor` in production and `FakeHypervisor` in test fixtures.
- Preserve the Pure Core / Imperative Shell architecture: planners, template synthesis engines, and XML AST manipulations remain pure functions with zero side effects.

## Testing Decisions

### What Makes a Good Test
Tests evaluate observable external behavior and contracts, not internal implementation details or private execution traces. Tests must assert on actual filesystem artifacts (using real `qemu-img` in temporary directories) and observable hypervisor domain state in memory, rather than asserting on method call counts, argument sequences, or recorded action lists. Tests should survive internal refactorings without modification as long as observable state transitions remain unchanged.

### Modules Tested
- **Concrete Storage Manager**: Verify copy-on-write overlay allocation, rebase operations, image integrity inspection, thin backup conversion, and safe file moves against real disk images in temporary directories.
- **Lifecycle Applier & Destroyer**: Verify domain creation, overlay provisioning, XML updates, recreation, and deprovisioning using the in-memory hypervisor fake and real temporary storage.
- **Domain Lifecycle Planner**: Verify calculation of plan actions based on hypervisor queries and image inspection against concrete storage.
- **Disaster Recovery**: Verify end-to-end backup and restore round-trips using real QEMU disk compression and in-memory hypervisor state.

### Prior Art
- `packages/onehost/tests/storage_toolchain_tests.rs`: Exercises unprivileged real `qemu-img` commands against real temporary files.
- `packages/onehost/tests/domain_element_tests.rs`: Tests XML AST manipulation purely in-process with zero test doubles.

## Out of Scope
- Modifying domain template XML generation or Nix packaging.
- Modifying instance manifest schemas or CLI command flags.
- Modifying core domain XML diffing algorithms or planning heuristics.
- Modifying or implementing feature tickets in `.scratch/onehost-declarative-reconciliation/`.

## Further Notes
- Governed by amended [ADR-0010 (Functional Rust Idioms and Pure Core Architecture)](docs/adr/0010-functional-rust-idioms.md) and repository coding standards in [`CODING_STANDARDS.md`](CODING_STANDARDS.md).
- Acts as a foundational prefactoring step that simplifies and unblocks subsequent declarative reconciliation feature tickets.
