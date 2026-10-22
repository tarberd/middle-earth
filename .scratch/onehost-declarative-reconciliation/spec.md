# Onehost Declarative Windows KVM Reconciliation

Status: ready-for-agent

## Problem Statement

Managing Windows libvirt KVM instances as infrastructure-as-code currently feels disjointed and incomplete compared to OpenTofu-managed container workflows. Base image preparation still requires manual out-of-band orchestration, multiple CD-ROM drives (such as VirtIO drivers) cannot be declared arbitrarily in the manifest, image identifiers rely on fragile informal build version strings and redundant namespaces, and modifying or removing virtual machines from the manifest does not automatically reconcile the live hypervisor state. Furthermore, attempts to apply configuration changes while virtual machines are actively executing risk corrupting disk overlays and guest filesystems.

## Solution

A fully autonomous, declarative reconciliation engine within `onehost` that manages the entire lifecycle of local Windows KVM instances on Gandalf. The solution unifies base image creation and instance reconciliation into a single seamless `apply` pipeline:
- Manifests declare flavors with an OEMDRV derivation and an arbitrary map of extra ISOs (such as VirtIO drivers), eliminating hardcoded driver logic.
- Images are uniquely identified by their canonical Microsoft UUP release tuple (`uup_id`, `edition`, `language`) and flavor, removing magic build version strings.
- `onehost apply` automatically detects unbuilt base images, retrieves missing UUP release packages using an internalized download toolchain, and constructs the Base Image in headless QEMU before creating instance overlays.
- A strict hypervisor quiescence invariant prevents mutations if any managed domain is running.
- Libvirt XML metadata tagging enables stateless detection and complete deprovisioning of orphan domains removed from the manifest, eliminating external state files.
- Redundant guardrail configurations are eliminated in favor of explicit operator review of the execution plan.

## User Stories

1. As a systems administrator, I want to declare a Windows instance in a single manifest, so that its virtual hardware, storage overlay, and base image are reconciled declaratively.
2. As a systems administrator, I want `onehost apply` to detect when a required base image does not exist in the depot store, so that I do not have to manually run an image build command beforehand.
3. As a systems administrator, I want `onehost` to automatically fetch and convert official Microsoft UUP dump packages into raw installation ISOs if they are missing from the depot ISO cache, so that installation media provisioning is completely automated.
4. As a workstation operator, I want images to be identified by their canonical UUP release tuple (`uup_id`, `edition`, `language`) and flavor, so that image references are unambiguous and unforgeable without relying on informal aliases.
5. As a workstation operator, I want image references to omit redundant `<os_name>` namespaces, so that manifest declarations remain concise and focused.
6. As a systems administrator, I want to attach arbitrary ISOs (such as `virtio-win.iso`) to a flavor declaration in the manifest, so that Windows PE and secondary setup scripts can access necessary drivers without hardcoded paths in the builder binary.
7. As a systems administrator, I want `onehost` to compute the content hash of a flavor dynamically from the contents of the OEMDRV media and all declared extra ISOs, so that the manifest does not require manually maintained hash fields.
8. As a developer, I want `onehost` to mount declared extra ISOs in deterministic alphabetical order during headless QEMU image builds, so that drive letter assignments in Windows PE remain completely predictable across runs.
9. As an operator running an image build, I want real-time guest serial (`COM1`) messages streamed to stderr with a distinct prefix, so that I can monitor Windows setup and PowerShell provisioning progress without attaching a graphical viewer.
10. As an operator running an image build, I want raw terminal reset escape sequences sanitized from guest serial output, so that host terminal display settings are not corrupted by the guest.
11. As a systems administrator, I want `onehost apply` to immediately fail if any managed domain is running or paused, so that disk modifications and libvirt reconfigurations never run against active virtual machines.
12. As a systems administrator, I want `onehost` to embed custom metadata into every defined libvirt domain XML, so that managed domains can be recognized on the hypervisor without maintaining an external state file.
13. As an operator who removes an instance from the manifest, I want `onehost plan` to detect the absent domain as an orphan, so that I am informed that the virtual machine will be cleaned up.
14. As an operator who approves a plan with orphan domains, I want `onehost apply` to completely deprovision them by undefining the domain, removing NVRAM variables, and unlinking their copy-on-write overlay disks, so that abandoned virtual machines do not consume storage or hypervisor resources.
15. As a systems administrator, I want `onehost` to acquire an exclusive file lock during mutating commands, so that concurrent executions cannot race or corrupt storage overlays.
16. As a workstation user, I want `onehost plan` to explicitly show when an instance overlay will be replaced due to base image drift, so that I can make an informed decision before approving recreation.
17. As a systems administrator, I want to eliminate redundant `lifecycle` policy blocks (`prevent_destroy` and `on_image_change`), so that manifest definitions are minimal and plan approval serves as the single authoritative confirmation boundary.
18. As a developer, I want `OnehostPlan` to separate actions into a de-duplicated prerequisite image build phase followed by instance reconciliation actions, so that shared base images are only built once even if referenced by multiple instances.
19. As a systems administrator, I want raw installation ISOs stored deterministically as `<depot_iso_dir>/<uup_id>-<edition>-<language>.iso`, so that installation media is easily discovered and cached.
20. As a systems administrator, I want base images stored deterministically as `<depot_store_dir>/<uup_id>-<edition>-<language>-<flavor>-<content_hash>.qcow2` with read-only permissions, so that base images are immutable and content-addressed.
21. As a systems administrator, I want `storage.ovmf_code` and `storage.nvram_template` in the manifest to be the authoritative source of UEFI firmware paths, so that domain template XMLs do not need hardcoded host paths.
22. As an operator, I want every instance to explicitly declare its target storage pool, so that storage locations are never ambiguous or reliant on hidden default values.
23. As a user, I want virtual machines to remain shut off after apply until explicitly started, so that background services do not start unexpectedly on Gandalf.

## Implementation Decisions

### 1. Manifest Schema Simplification & Data Contract
- **Flavor Configuration**: Holds an absolute path to the OEMDRV ISO and a map of named extra ISOs (`extra_isos: HashMap<String, PathBuf>`). The redundant `hash` field is removed.
- **Instance Configuration**: Contains `uuid`, `template_xml`, explicit `pool` name, and a structured `image` specification. The nested `lifecycle` block (`on_image_change`, `prevent_destroy`) and the `autostart` attribute are removed.
- **Image Specification**: Defined as a structured object containing `uup_id` (RFC-4122 string), `edition` (normalized string), `language` (normalized BCP-47 tag), and `flavor` (string referencing a declared flavor). On the CLI, this is accepted as a 4-segment path: `<uup_id>/<edition>/<language>/<flavor>`.
- **Storage Configuration**: Retains explicit paths for `depot_store_dir`, `depot_iso_dir`, `depot_backup_dir`, `nvram_dir`, `nvram_template`, and `ovmf_code`. The `default_pool` attribute is removed.

### 2. Dynamic Content Hashing
- `onehost` computes a streaming SHA-256 digest across the bytes of `oemdrv_path` followed by each file in `extra_isos` (sorted alphabetically by key).
- The first 12 hexadecimal characters of this digest form the `<content_hash>`.
- Base images are resolved as:
  `<depot_store_dir>/<uup_id>-<edition>-<language>-<flavor>-<content_hash>.qcow2`.

### 3. Autonomous Base Image Build Pipeline
- `onehost apply` pre-flight checks whether the resolved base image path exists in the depot store.
- If absent, it verifies whether `<depot_iso_dir>/<uup_id>-<edition>-<language>.iso` exists.
- If the ISO is missing, `onehost` executes its internalized UUP dump fetcher toolchain (using packaged `aria2`, `wimlib`, and `cabextract` dependencies), downloading and converting the packages in an isolated scratch directory with atomic `.tmp` staging.
- Headless QEMU is launched with the Windows ISO, OEMDRV ISO, and all `extra_isos` attached as read-only CD-ROM drives.
- Guest serial (`COM1`) output is piped, sanitized of terminal reset sequences, and formatted to stderr with a `[guest-serial]` tag.
- Once Sysprep completes and QEMU powers off, the scratch disk is compressed, verified for integrity, and atomically promoted to the depot store with `0444` permissions.

### 4. Hypervisor Quiescence & Execution Invariant
- Before planning or applying any state changes, `onehost` inspects all declared instances and known managed domains on the hypervisor.
- If any managed domain has an active state (`Running`, `Paused`, etc.), execution halts immediately with a `HypervisorNotQuiescentError`.
- No automatic ACPI shutdown or partial reconciliation is attempted.

### 5. Stateless Orphan Discovery and Purging
- When synthesizing concrete domain XML, `onehost` injects:
  `<metadata><onehost:instance xmlns:onehost="https://middle-earth.internal/onehost" managed="true"/></metadata>`.
- During planning, `onehost` queries libvirt for all existing domains and inspects their XML metadata.
- Any domain tagged as `onehost`-managed that does not appear in `manifest.instances` is scheduled as an orphan purge action.
- When applied, orphan domains are deprovisioned by undefining the domain with `--nvram` and unlinking their copy-on-write overlay disks.

### 6. Process Concurrency Safety
- An exclusive advisory lock (`flock`) on `$XDG_STATE_HOME/onehost/onehost.lock` is acquired at the start of all mutating subcommands (`apply`, `destroy`, `image build`).
- If another process holds the lock, execution aborts immediately with a lock conflict error.

## Testing Decisions

### What Makes a Good Test
Tests must evaluate observable external system behavior and contracts, not internal implementation details or private routines. In addition to testing state machine logic, tests must execute against real system tools (`qemu-img`, real filesystem I/O, `virt-xml-validate`, and real `virsh`) to guarantee that operations perform reliably in the real world rather than solely within simulated in-memory mocks.

### Testing Seams

#### Seam 1: Real-Tool Storage & Process Seam (`QemuImgStorage` + Real Filesystem + `virt-xml-validate`)
- **Seam Location**: The boundary between `onehost` and host system tools.
- **Components Tested**: `QemuImgStorage`, `ProcessRunner`, and filesystem I/O on a real temporary directory (`tempdir`).
- **Real-World Behaviors Validated**:
  - Real `qemu-img create -F qcow2 -b ...` backing chains and copy-on-write overlay allocation.
  - Real `qemu-img info` JSON parsing, backing file verification, and virtual/actual size queries.
  - Real atomic staging (`.tmp` to final) and strict `0444` read-only permission enforcement.
  - Real advisory host process locking (`flock`) on `$XDG_STATE_HOME/onehost/onehost.lock`.
  - Real XML schema validation via `virt-xml-validate` against official Libvirt RNG schemas.

#### Seam 2: Real Libvirt Integration Seam (`VirshHypervisor` on `test:///default`)
- **Seam Location**: The boundary between `onehost` and Libvirt hypervisor daemon.
- **Components Tested**: `VirshHypervisor` executing real `virsh` commands against Libvirt's hermetic test driver (`virsh -c test:///default`).
- **Real-World Behaviors Validated**:
  - Real domain definitions with embedded `<metadata><onehost:instance managed="true"/></metadata>` blocks.
  - Real hypervisor domain enumeration (`virsh list --all`), state queries (`virsh dominfo`), and XML dumping (`virsh dumpxml`).
  - Real stateless orphan discovery: detecting domains carrying onehost metadata that are missing from the current manifest.
  - Real domain deprovisioning (`virsh undefine --nvram`).
  - Strict hypervisor quiescence rejection when a domain is in an active state.

### Scenarios Covered
1. **End-to-End Image & Instance Creation**: Manifest with an unbuilt base image plans both an image build action and instance creation; apply builds the image, creates real qcow2 backing chains, and defines the domain in Libvirt with metadata.
2. **Missing UUP ISO Fetch**: Image build with a missing raw ISO triggers the UUP download toolchain with atomic `.tmp` staging before launching QEMU.
3. **Hypervisor Quiescence Rejection**: Active domain state causes `onehost apply` to immediately abort without executing planned disk or domain actions.
4. **Base Image Drift & Overlay Recreation**: Updating an extra ISO or OEMDRV changes the computed content hash, causing the planner to schedule replacement of the instance overlay disk.
5. **Stateless Orphan Purging**: Removing an instance from the manifest causes the planner to query libvirt for `<onehost:instance>` metadata and schedule complete deprovisioning (undefine + delete overlay).
6. **Concurrent Process Lock Contention**: Running concurrent mutating commands respects the host file lock and aborts with a clean error.
7. **XML Schema Compliance**: Generated concrete XML passes official `virt-xml-validate` checks.

### Prior Art
- `packages/onehost/tests/disaster_recovery_integration_tests.rs`: Multi-disk overlay recovery with real filesystem staging and `QemuImgStorage`.
- `packages/onehost/tests/pipeline_integration_tests.rs`: Hermetic multi-instance lifecycle testing.
- `packages/onehost/tests/lifecycle_tests.rs`: Drift detection and planner action synthesis.

## Out of Scope
- Public internet routing through Sauron or network bridging to `br-public-hosts` (instances are strictly local workstation VMs).
- Headless server virtualization profiles without GPU passthrough or Looking Glass.
- High-level JSON abstraction layers for libvirt virtual hardware topology (Nix continues generating raw Domain Template XML).
- Autostart management via libvirt symlinks.
- Background live VM migrations or live memory snapshots.

## Further Notes
- ADRs governing this feature: [0001](file:///home/tarberd/middle-earth/docs/adr/0001-onehost-for-windows-kvm.md) through [0009](file:///home/tarberd/middle-earth/docs/adr/0009-plan-confirmation-over-lifecycle-guardrails.md).
- Canonical terminology defined in [`GLOSSARY.md`](file:///home/tarberd/middle-earth/GLOSSARY.md).
