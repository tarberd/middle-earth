# 06: Purge Mock Hypervisor from Production Sources

**What to build:** Deletes the mock hypervisor and recorded hypervisor action enums from production crate modules and public exports. Cleans up hypervisor tests so that only production hypervisor implementations and domain state parsing are tested within production test targets.

**Blocked by:** 05: Migrate Planner and Lifecycle Tests to Fake Hypervisor State Assertions

**Status:** resolved

- [x] Mock hypervisor and recorded hypervisor action enums are deleted from production sources.
- [x] Production library exports no longer expose testing hypervisor doubles or call-recording types.
- [x] Hypervisor tests only exercise production adapters and data parsing.
