# Findings & Architecture: Windows VM Provisioning Refactoring to Onehost

## Requirements & Scope
- **Goal**: Refactor the Nix flake Windows VM provisioning to rely exclusively on the newly created `onehost` Rust CLI tool, eliminating all legacy bash scripts, deprecated domain XML generators, obsolete workarounds, and dead code.
- **Backwards Compatibility**: **STRICTLY PROHIBITED**. The user explicitly mandated: *"we MUST NOT keep backwards compatibility, we are free to refactor without this constraint."*
- **Source of Truth**: `packages/onehost` is the unified, declarative lifecycle manager for Windows VMs. The Nix flake compiles declarations (`instances.nix`), OEMDRV assets (`windows.nix`), and domain hardware templates (`onehost.nix`) into `onehost.json`.

---

## Inventory of Affected Files

| File | Role | Action Required |
|:---|:---|:---|
| [`middle-earth/hosts/gandalf/virtualization/images/windows.nix`](file:///home/tarberd/middle-earth/middle-earth/hosts/gandalf/virtualization/images/windows.nix) | Contains Windows unattended assets AND 730+ lines of dead legacy bash scripts & XML builders | **Major Refactor**: Strip all dead scripts (`buildApp`, `provisionApp`, `backupApp`, `mkWin11Domain`, `domainXmls`, `provisionInstance`, `instanceForVersion`, etc.). Retain only pure asset generators (`autounattendXml`, `sysprepXml`, `provisionPs1`, `errorHandlerCmd`) and optional clean UUP ISO builder app. |
| [`middle-earth/hosts/gandalf/virtualization/images/windows-versions.nix`](file:///home/tarberd/middle-earth/middle-earth/hosts/gandalf/virtualization/images/windows-versions.nix) | Declarative metadata for Windows versions (UUP ID, edition, language) | **Retain**: Clean declarative map used by ISO builder and image versioning. |
| [`middle-earth/hosts/gandalf/virtualization/images/default.nix`](file:///home/tarberd/middle-earth/middle-earth/hosts/gandalf/virtualization/images/default.nix) | Submodule loader for `windows-versions` and `windows` | **Retain**: Exports clean `windows` module. |
| [`middle-earth/hosts/gandalf/virtualization/onehost.nix`](file:///home/tarberd/middle-earth/middle-earth/hosts/gandalf/virtualization/onehost.nix) | Flake module compiling `onehost.json`, domain template XMLs, OEMDRV ISO derivations, and `onehost-*` apps | **Refactor**: Add `buildIso` app (`onehost-build-iso`) to `apps` set. Ensure clean consumption of `windows` assets. |
| [`apps/default.nix`](file:///home/tarberd/middle-earth/apps/default.nix) | Top-level flake apps exposed to `nix run .#<app>` | **Refactor**: Eradicate legacy backward-compatibility shims (`build-windows-image`, `provision-windows-vm`, `backup-windows-vm`). Expose modern `onehost-*` apps. |
| [`middle-earth/hosts/gandalf/virtualization/kvm.nix`](file:///home/tarberd/middle-earth/middle-earth/hosts/gandalf/virtualization/kvm.nix) | Host Libvirt service & nightly backup timer | **Audit**: Already points to `${onehost.apps.backup}/bin/onehost-backup all`. |
| [`middle-earth/hosts/gandalf/virtualization/instances.nix`](file:///home/tarberd/middle-earth/middle-earth/hosts/gandalf/virtualization/instances.nix) | Declarative instance specifications (`win11-gollum`, `win11-beruthiel`) | **Audit**: Already modernized with clean 3-segment tags and explicit lifecycle policies. |

---

## Detailed Dead Code Audit in `windows.nix` (Total ~730 lines)

1. **`buildApp` (`build-windows-image`, lines 431–636, ~206 lines)**:
   - Legacy monolithic bash script handling UUP dump download, swtpm startup, headless QEMU launch, VNC monitor sendkey loop, sysprep wait, and `qemu-img convert`.
   - **Reason for removal**: Fully superseded by `onehost image build` (`packages/onehost/src/image/builder.rs`), which executes this hermetically using structured RAII cleanup guards, content-addressed tags, and strict error handling.
2. **`mkWin11Domain` and `domainXmls` (lines 638–864, ~227 lines)**:
   - Hardcoded Libvirt Domain XML generation with hardcoded disk paths (`/var/lib/libvirt/images/${name}.qcow2`), missing `xmlns:onehost` and `<disk onehost:role='os-disk'/>` markers.
   - **Reason for removal**: Fully superseded by `onehost.nix`'s `mkDomainTemplateXml` and `templateXmls`, which use OpenTofu-style domain templating with explicit role annotations.
3. **`provisionInstance`, `provisionAllScript`, `provisionTargetsScript`, `provisionApp` (`provision-windows-vm`, lines 866–1010, ~145 lines)**:
   - Bash script checking depot master existence, copying to local base, calling `qemu-img create` for CoW overlay, copying NVRAM template, and running `virsh define`.
   - **Reason for removal**: Fully superseded by `onehost apply` (`packages/onehost/src/lifecycle/applier.rs`), which handles declarative diff reconciliation, safe storage pool relocation, guardrail enforcement (`on_image_change`), and NVRAM initialization.
4. **`backupAllScript`, `backupApp` (`backup-windows-vm`, lines 1012–1124, ~113 lines)**:
   - Bash script running `virsh snapshot-create-as`, `qemu-img convert`, and `virsh blockcommit`.
   - **Reason for removal**: Fully superseded by `onehost backup` (`packages/onehost/src/backup/engine.rs`), which features multi-disk atomic snapshots, opportunistic VSS quiescing via `qemu-ga`, RAII pivot cleanup guards, and structured `manifest.json` generation.
5. **`instanceForVersion` and `hostnameForVersion` (lines 26–46, ~21 lines)**:
   - Helper looking up the first instance name matching a version key to hardcode the hostname into `autounattend.xml` and `sysprep.xml`.
   - **Reason for removal**: Golden master images are generalized (`sysprep /generalize`). Baking an individual VM's hostname into the golden image violates image generalization and content-addressed reusability. A clean generic computer name (`WIN11-MASTER` or `WIN11-VM`) is standard.
6. **`versionLookupScript` (lines 412–429, ~18 lines)**:
   - Bash case statement used by the legacy `buildApp`.
   - **Reason for removal**: Dead code along with `buildApp`.

---

## Retained & Modernized Assets in `windows.nix`

1. **`mkAutounattendXml` / `autounattendXml`**:
   - Generates Windows PE unattended installation answer file (`autounattend.xml`) with partition layout (EFI 512MB, MSR 16MB, NTFS OS disk), bypass checks for TPM/RAM/CPU/Storage/SecureBoot, VirtIO driver paths, and setup error handler hooks.
2. **`mkSysprepXml`**:
   - Generates post-sysprep OOBE answer file (`sysprep.xml`) automating local admin account creation and skipping user OOBE screens.
3. **`provisionPs1`**:
   - PowerShell provisioning script run on first logon to install VirtIO Guest Tools, Looking Glass Host (if present in OEMDRV), tune power/sleep settings, and initiate generalization via `sysprep /generalize /oobe /shutdown`.
4. **`errorHandlerCmd`**:
   - Batch script dumping Windows Panther setup logs to `COM1` upon fatal installation errors for headless debugging.
5. **`uupEnv` and `buildIsoApp` (`onehost-build-iso`)**:
   - Clean, isolated FHS environment (`uupEnv`) and shell application to download and assemble the official Windows installation ISO from Microsoft update servers via UUP dump directly into `/data/depot/virtualization/libvirt/iso/win11-${version}.iso`.

---

## Flake Apps Modernization in `apps/default.nix`

### Removed Deprecated Shims:
- `build-windows-image`
- `provision-windows-vm`
- `backup-windows-vm`

### Standard Modern Apps:
- `onehost`: Direct CLI access (`${onehostPkg}/bin/onehost`)
- `onehost-plan`: Reconcile declared state against live hypervisor
- `onehost-apply`: Reconcile instances, cache base images, init NVRAM, define domains
- `onehost-destroy`: Graceful ACPI shutdown and deprovisioning with guardrails
- `onehost-backup`: Live thin snapshots with RAII pivot cleanup
- `onehost-restore`: Single-command disaster recovery
- `onehost-status`: Live instance status inspection
- `onehost-build-image`: Automated golden master builder with headless QEMU
- `onehost-build-iso`: Unattended Windows ISO builder via UUP dump
