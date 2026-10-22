# 04: Shared State-Based Fake Hypervisor Test Fixture

**What to build:** Implements a shared in-memory fake hypervisor test double in test fixtures implementing the hypervisor trait. The fake models domain presence, defined XML, running states, block device attachments, and storage pool paths purely in memory, exposing state-based inspection helpers for test assertions with zero action-recording spies.

**Blocked by:** 03: Collapse Storage Trait into Concrete QEMU Storage Manager and Delete Mock

**Status:** ready-for-agent

- [ ] In-memory fake hypervisor fixture implements the hypervisor boundary port in test fixtures outside production crate sources.
- [ ] Domain state, XML configurations, block devices, and storage pools are maintained in memory without call-recording spies.
- [ ] Inspection helpers allow tests to query defined domains, XML bodies, and active states.
