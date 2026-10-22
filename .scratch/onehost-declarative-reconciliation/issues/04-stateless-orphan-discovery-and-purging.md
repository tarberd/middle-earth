# 04: Stateless Orphan Domain Discovery and Purging

**What to build:** Enables clean deprovisioning of virtual machines removed from the declarative manifest without external state files. During planning, onehost enumerates hypervisor domains to find any domain tagged with onehost metadata that is missing from the manifest, scheduling it for purging. When applied, onehost deprovisions the orphan domain by undefining it with NVRAM cleanup and unlinking its copy-on-write overlay disk. All instance deprovisioning (both explicit `destroy` and orphan purging) consistently unlinks the overlay disk. `onehost status` displays unmanaged hypervisor domains informatively without reporting an error.

**Blocked by:** 03: Libvirt Domain Metadata Tagging and Hypervisor Quiescence Check

**Status:** ready-for-agent

- [ ] `onehost plan` identifies domains bearing onehost metadata not declared in the manifest as orphan domains scheduled for deprovisioning.
- [ ] `onehost apply` undefines orphan domains with `--nvram` and unlinks their copy-on-write overlay disks from storage.
- [ ] `onehost destroy` consistently unlinks the overlay disk along with undefining the domain.
- [ ] `onehost status` displays non-managed hypervisor domains informatively without treating them as an error.
