# Progress Log: Windows VM Provisioning Refactoring to Onehost

## Session: 2026-09-27

### Current Status
- **Phase:** Phase 6: Upstream Synchronization & Delivery
- **Started:** 2026-09-27
- **Plan ID:** `2026-09-27-nix-flake-windows-vm-refactor-onehost`

---

### Actions Taken
- Initialized dedicated planning session using `planning-with-files` skill (`scripts/init-session.sh`).
- Confirmed user directive: **STRICTLY ZERO BACKWARDS COMPATIBILITY**; free to refactor cleanly without legacy constraints.
- Executed Phase 1: Requirements Discovery & Dead Code Inventory:
  - Scanned and mapped all Windows virtualization code across `middle-earth/hosts/gandalf/virtualization/`, `packages/onehost/`, and `apps/default.nix`.
  - Audited `middle-earth/hosts/gandalf/virtualization/images/windows.nix` (1140 lines) and identified over 730 lines of completely dead legacy code:
    - `buildApp` (`build-windows-image` monolithic bash script, lines 431–636, ~206 lines)
    - `mkWin11Domain` and `domainXmls` (obsolete Libvirt Domain XML generators, lines 638–864, ~227 lines)
    - `provisionInstance`, `provisionAllScript`, `provisionTargetsScript`, `provisionApp` (`provision-windows-vm` bash script, lines 866–1010, ~145 lines)
    - `backupAllScript`, `backupApp` (`backup-windows-vm` bash script, lines 1012–1124, ~113 lines)
    - `instanceForVersion`, `hostnameForVersion`, and `versionLookupScript` (obsolete VM hostname injection into golden images and lookup case statement, lines 26–46, 412–429, ~39 lines)
  - Audited `apps/default.nix` and identified dead backward-compatibility shims: `build-windows-image`, `provision-windows-vm`, `backup-windows-vm`.
  - Verified existing Windows ISOs in `/data/depot/virtualization/libvirt/iso/` and created clean symlinks (`win11-26300.9457.pro.en-us.iso`, `win11-26300.9457.pro.ja-jp.iso`) conforming to `onehost`'s naming standard.
  - Documented findings, dead code inventory, and modern architecture in `findings.md`.
  - Authored comprehensive `task_plan.md` incorporating the full 3-pillar engineering standards and 6 execution phases.
- Executed Phase 2: Target Flake Module Architecture & Design Specification:
  - Specified lean module interface for `windows.nix` exporting only pure OEMDRV assets (`autounattendXml`, `mkSysprepXml`, `provisionPs1`, `errorHandlerCmd`) and `uupEnv` / `buildIsoApp`.
  - Specified `onehost.nix` and `apps/default.nix` integration removing all legacy shims and exposing `onehost-build-iso`.
- Executed Phase 3: Refactor Windows Module (`windows.nix` & `windows-versions.nix`):
  - Refactored `middle-earth/hosts/gandalf/virtualization/images/windows.nix`: pruned 722 lines of dead bash scripts, legacy XML generators, and obsolete hostname helpers.
  - Preserved pure unattended installation assets (`mkAutounattendXml`, `autounattendXml`, `mkSysprepXml`, `provisionPs1`, `errorHandlerCmd`).
  - Implemented clean `buildIsoApp` (`onehost-build-iso`) downloading UUP dump packages and building `win11-${version}.iso` directly into `/data/depot/virtualization/libvirt/iso/`.
  - Updated `createFlakeModule` in `windows.nix`.
- Executed Phase 4: Flake App Eradication & Modernization in `apps/default.nix` & `onehost.nix`:
  - Added `buildIso = windows.buildIsoApp;` to `apps` in `onehost.nix`.
  - Refactored `apps/default.nix`: purged `build-windows-image`, `provision-windows-vm`, and `backup-windows-vm`. Added `onehost-build-iso`.
  - Verified `nix flake show` displays clean application list.
- Executed Phase 5: Verification, Nix Flake Check & Live Hypervisor Validation:
  - Verified hermetic package build: `nix build .#packages.x86_64-linux.onehost --no-link` (SUCCESS).
  - Verified manifest derivation: `nix build .#packages.x86_64-linux.onehost-manifest --no-link` (SUCCESS).
  - Executed full test suite: `cargo test --all-targets` (140/140 passed).
  - Executed Clippy audit: `cargo clippy --all-targets -- -D warnings` (0 warnings).
  - Executed Nix flake check: `nix flake check` (`all checks passed!` across all configurations, packages, and apps).
  - Executed live hypervisor checks: `nix run .#onehost-plan` and `nix run .#onehost-status` on Gandalf.
  - Executed static invariant audit: 0 loops, 0 unwraps, 0 single-letter closures, 0 `let _ =` swallows.

---

### Verification Matrix
| Tier / Gate | Verification Check | Expected | Actual | Status |
|:---|:---|:---|:---|:---|
| Inventory Audit | Identify all dead code in `windows.nix` | ~730 lines identified | 730+ lines cataloged in findings.md | PASS |
| App Audit | Identify all legacy shims in `apps/default.nix` | 3 shims identified | `build-windows-image`, `provision-windows-vm`, `backup-windows-vm` | PASS |
| Code Pruning | Eradicate dead bash & XML code | ~700+ lines removed | 722 lines deleted via git diff | PASS |
| Flake Show | Modern apps evaluate cleanly | All 15 apps evaluate | Evaluated cleanly with zero errors | PASS |
| Nix Package Build | `packages.x86_64-linux.onehost` | Builds cleanly | Successfully built via Nix | PASS |
| Manifest Derivation | `packages.x86_64-linux.onehost-manifest` | Builds cleanly | Successfully built via Nix | PASS |
| Test Suite (Tier 1 & 2) | `cargo test --all-targets` | 140 tests pass | 140 passed, 0 failed, 0 warnings | PASS |
| Clippy Audit (Tier 3) | `cargo clippy -- -D warnings` | Zero warnings | 0 warnings | PASS |
| Flake Check | `nix flake check` | All checks pass | `all checks passed!` | PASS |
| Live Plan | `nix run .#onehost-plan` | Validates live state | Clean OpenTofu-style plan generated | PASS |
| Live Status | `nix run .#onehost-status` | Displays VM table | Correctly formats Gandalf VM table | PASS |
| Live ISO App | `nix run .#onehost-build-iso` | Verifies existing ISO | Cleanly identifies cached ISO | PASS |
| Static Invariant Audit | 0 loops, 0 unwraps, 0 single-letter, 0 let _ = | 100% compliant | 100% compliant | PASS |

---

### Errors Encountered
| Error | Attempt | Resolution |
|:---|:---|:---|
| Syntax error in `apps/default.nix` due to accidentally omitted closing brace | 1 | Restored closing `};` on `build-palworld-image` |
