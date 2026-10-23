# 09: Pure State Snapshot and Running-State Lifecycle Transitions in FakeHypervisor

**What to build:** Refactors the shared fake hypervisor test double to follow a purely functional state-snapshot model and enforce realistic running-state lifecycle transitions. Replaces individual OOP getter methods on the fake hypervisor with a single `snapshot()` projection returning an immutable state value with public fields, preserves existing running state when redefining domain XML, rejects undefining running domains until they are shut down or destroyed, and replaces side-effecting fold calls with value-producing iterator combinators.

**Blocked by:** 08: Collapse Process Runner Trait and Delete Process Mock

**Status:** resolved

- [x] `FakeHypervisor` replaces individual field-getter methods with a single `snapshot()` method returning an immutable `FakeHypervisorState` value with public fields, and all test assertions inspect the snapshot directly.
- [x] Redefining an existing domain preserves its current `DomainState` (`Running` stays `Running`), while newly defined domains initialize as `Shutoff`.
- [x] Undefining a `Running` domain returns a `HypervisorError`, requiring callers to shut down or destroy the domain before undefining it.
- [x] External snapshot creation in `FakeHypervisor` uses value-producing iterator combinators (`collect`, `map`, `chain`) with zero side-effecting `fold((), ...)` calls.
