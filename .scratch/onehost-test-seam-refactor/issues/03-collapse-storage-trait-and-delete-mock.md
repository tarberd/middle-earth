# 03: Collapse Storage Trait into Concrete QEMU Storage Manager and Delete Mock

**What to build:** Collapses the storage trait abstraction into the concrete QEMU disk image storage manager, eliminating generic storage parameters from lifecycle appliers, destroyers, planners, backup engines, and CLI dispatchers. Deletes the in-memory mock storage manager from production sources, leaving a single concrete storage manager across production and test code.

**Blocked by:** 02: Physical Disk State Verification in Lifecycle and Disaster Recovery Tests

**Status:** resolved

- [x] Generic storage type parameter is removed from lifecycle appliers, destroyers, planners, backup engines, restore engines, and CLI entry points.
- [x] Storage trait is collapsed into the concrete QEMU storage manager.
- [x] In-memory mock storage manager and recorded storage action enums are completely removed from production sources.
- [x] Full test suite compiles cleanly and passes without warnings.
