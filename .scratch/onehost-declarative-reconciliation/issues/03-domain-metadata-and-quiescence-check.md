# 03: Libvirt Domain Metadata Tagging and Hypervisor Quiescence Check

**What to build:** Ensures onehost-managed domains are identifiable on the hypervisor and that mutations never occur against active virtual machines. When synthesizing Domain Template XML, onehost injects custom metadata identifying the domain as managed. Before executing plan or apply operations, onehost inspects declared instances and all onehost-tagged domains on the hypervisor; if any domain is running or paused, execution halts immediately with a hypervisor quiescence error.

**Blocked by:** 01: Manifest Schema Simplification, Canonical Release Tuple, and Dynamic Content Hashing

**Status:** ready-for-agent

- [ ] Synthesized concrete domain XML includes the `<metadata><onehost:instance xmlns:onehost="https://middle-earth.internal/onehost" managed="true"/></metadata>` block.
- [ ] Generated domain XML with metadata conforms to libvirt schema requirements.
- [ ] Hypervisor abstraction provides domain enumeration across all defined domains on the host.
- [ ] Planning or applying against a hypervisor where any declared instance or onehost-tagged domain is running or paused halts immediately with a hypervisor quiescence error.
