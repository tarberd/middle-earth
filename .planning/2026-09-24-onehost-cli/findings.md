# Findings & Architectural Specification

## Requirements Summary
- Develop a new CLI tool called `onehost` in Rust located at [`packages/onehost`](file:///home/tarberd/middle-earth/packages/onehost).
- Its purpose is to manage the lifecycle of Windows libvirt KVM deployments.
- Must be configured by Nix flake derivations just like OpenTofu incus containers.
- Must follow XDG Base Directory specifications (`$XDG_CONFIG_HOME`, `$XDG_DATA_HOME`, `$XDG_STATE_HOME`, `$XDG_CACHE_HOME`, `$XDG_RUNTIME_DIR`).
- Must avoid non-declarative behavior / avoid default values / avoid ambiguity.
- Must be developed using TDD (Test-Driven Development).
- Refactor behaviors from existing bash scripts (`build-windows-image`, `provision-windows-vm`, `backup-windows-vm`).

### Mandatory Design Guidelines & Engineering Standards

#### North Star Architectural Vision
> **"A Functional Core with an Imperative Shell, orchestrating declarative, idempotent, and hermetic Windows Libvirt/KVM lifecycles via OpenTofu-style reconciliation."**

#### Foundational Principle: Universal Code Equivalence & Absolute Standards
> **"NEVER assume code to be less critical or important to any task. All code across the entire codebase—whether pure domain logic, imperative execution shell, low-level parser routines, CLI dispatchers, error definitions, mock drivers, test harnesses, or integration suites—must be held to the exact same uncompromising, high quality standards during any task. There are zero second-class components, zero exemptions, and zero quality tiers."**

#### Pillar I: Architectural Blueprint & System Contracts
1. **The Boundary Contract: Functional Core vs. Imperative Shell**:
   - **The Functional Core (Pure Domain Logic)**:
     - *Scope*: AST parsing, validation, Domain Template XML transformation (`template.rs`), drift calculation (`diff.rs`), and content-addressed image tag/hash resolution (`tag.rs`).
     - *Contract*: Pure transformations `fn(Input) -> Result<Output, Error>`. Zero I/O, zero filesystem calls, zero network access, zero process execution. Tests for the Functional Core require **zero mocks**.
   - **The Trait Boundary (Contracts & Hardware Abstraction)**:
     - *Scope*: Abstract interfaces (`Hypervisor`, `StorageManager`, `ProcessRunner`).
     - *Contract*: Strictly decouples external infrastructure interactions. Production implementations wrap live CLI/system tools (`VirshHypervisor`, `QemuImgStorage`); test implementations provide hermetic in-memory mocks (`MockHypervisor`, `MockStorageManager`).
     - *Trait Purity Rule*: Traits must NOT embed default implementations with real host filesystem side-effects (e.g. raw `std::fs` calls). All I/O must be explicit in production implementors.
   - **The Imperative Shell (Reconciliation, Planning & Orchestration)**:
     - *Query Shell (`DomainLifecyclePlanner`)*: Queries traits to snapshot live hypervisor and storage state, delegating drift comparison and action generation to the pure Core engines, emitting an immutable `OnehostPlan`.
     - *Execution Shell (`DomainLifecycleApplier`, `DomainLifecycleDestroyer`, `BackupEngine`, `RestoreEngine`)*: Thin, linear orchestration sequentially executing the plan's actions via traits.
     - *Shell Trait Boundary Invariant*: The imperative shell must NEVER invoke `std::fs` or run external processes directly. All external mutations (including NVRAM directory creation and file copying) must be routed through trait methods (e.g. `StorageManager::initialize_nvram`).
     - *CLI Dispatch (`main.rs`, `cli.rs`)*: Parses command arguments and configures output streams.
2. **Mock Hermeticity (100% In-Memory Isolation)**:
   - Test mocks (`MockHypervisor`, `MockStorageManager`) must remain completely hermetic and isolated from the host environment.
   - Calling `std::fs::copy`, `std::fs::rename`, `std::fs::remove_file`, `std::fs::create_dir_all`, `std::fs::set_permissions`, or checking host disk existence via `Path::exists()` inside mocks is strictly prohibited.
   - Mocks must record actions in structured algebraic records (e.g. `RecordedStorageAction`, `RecordedHypervisorAction`) and track virtual state purely in-memory.
3. **Declarative Purity & Zero Hidden State**:
   - **Zero Implicit Defaults**: Every configuration value, hardware device, storage pool, path, and version must be explicitly declared or strictly derived. Missing or ambiguous fields fail fast during deserialization/validation.
   - **NO Backwards Compatibility**: The tool represents a modern, clean-slate standard. Legacy shims, deprecated tag structures, or historical migration paths are strictly prohibited.
4. **Reconciliation & Safety Invariants**:
   - **Total Idempotency**: Applying a plan to an already reconciled system produces a `NoOp` plan and performs zero mutations: `plan(apply(plan(state))) == NoOp`.
   - **Non-Destructive by Default (Guardrails)**: Destructive actions (recreating an instance overlay on base image drift, or deprovisioning an instance) are prohibited unless explicitly enabled in the manifest (`on_image_change: recreate`) or via CLI override flags (`--allow-recreate`, `--allow-destroy-protected`).
   - **Atomicity & RAII Resource Cleanup**: Any multi-step operation with transient external state (e.g. live thin snapshots, golden master promotion) must use RAII cleanup guards (e.g. pivoting blockcommits, staging to `.tmp` files) guaranteeing that cancellations, panics, or I/O errors cannot leave orphan `.snap` files or corrupted images.
5. **Structured Error & Diagnostics Philosophy**:
   - Strongly typed hierarchy defined via `thiserror` without dynamic string errors.
   - Categorized into:
     1. *Domain & Validation Errors*: Syntactic/semantic errors in user manifests or templates.
     2. *Infrastructure & I/O Errors*: Hypervisor command exits, `qemu-img` failures, or filesystem I/O errors.
     3. *Policy & Guardrail Violations*: Intentional aborts protecting data integrity.
   - **Zero Stringly-Typed Errors**: Do not emit dynamic strings in catch-all error variants (e.g. avoid `ConfigurationError { details: String }` when a typed variant like `InstanceNotDeclared` or `FlavorResolutionError::UnknownFlavor` can be used).
   - **Zero Silent Error Swallowing**: Never discard fallible external operations with `let _ =`. When operations have expected idempotent conditions (such as undefining a domain that may already not exist), explicitly match and handle the expected variant (e.g., `HypervisorError::DomainNotFound => Ok(())`) while bubbling up true infrastructure errors.
   - Actionable diagnostics: Error messages must provide precise diagnostic context (naming the specific instance, device, path, expected vs. actual state, and required override flags).
6. **CLI Stream Discipline & Structured Tracing**:
   - `stdout` is reserved exclusively for structured user/machine data (execution plans, diff summaries, JSON status exports).
   - `stderr` is reserved exclusively for diagnostic logging and telemetry via `tracing` (`tracing::info!`, `warn!`, `error!`, `debug!`).
   - Raw `println!` is strictly prohibited in library modules; all operational and progress events must flow through structured `tracing` spans and events.

#### Pillar II: Functional Rust Craft & Idiomatic Implementation
1. **Transparent Records & "Born-Valid" Values**:
   - **Algebraic Data Records**: Data models, AST nodes, diff results, and plan actions are transparent algebraic records with public fields (`pub field: Type`).
     - *Target Idiom*: Direct field access, destructuring by move (`let Foo { bar, baz } = foo;`), pattern matching, and struct update syntax (`Foo { bar: new_bar, ..self }`).
     - *Anti-Pattern*: Boilerplate OOP-style getter/setter functions (`fn field(&self) -> &Field`).
   - **Born-Valid Types (Parse, Don't Validate)**: Types enforce their structural invariants during construction (`new()`, `parse()`, `from_str()`). Once instantiated, a type is guaranteed valid; intermediate unvalidated states are prohibited (exemplified by `InstanceUuid` enforcing RFC-4122).
2. **Move Semantics & Pure Value Builders**:
   - **Strict `fn(self, ...) -> Self` Pattern**: Builders and value-transforming methods consume `self` by value, update fields in-place using struct update syntax (`..self`), and return the owned value.
     - *Target Idiom*: Ownership-driven functional transformations; if a borrower holding a reference needs a transformed value, the `.clone()` must be explicit at the call site.
     - *Anti-Pattern*: `fn(&self, ...) -> Self`, which hides internal deep clones under the guise of call-site convenience.
3. **Computation: Iterator Pipelines over Imperative Loops**:
   - Multi-item transformations, searches, filters, and collections must use declarative iterator pipelines or functional recursion.
     - *Target Idiom*: `.into_iter()`, `.iter()`, `.find()`, `.filter()`, `.map()`, `.try_for_each()`, `.fold()`, `.all()`, `.any()`, `.chain()`, `.flatten()`, `std::iter::from_fn()`, tail-recursive functional transformations.
     - *Anti-Pattern*: Imperative `for`, `while`, or `loop` statements with mutable accumulators or early returns.
4. **Control Flow: Expressions over Statements**:
   - Branching must be value-producing expressions that compute results directly.
     - *Target Idiom*: `let value = if cond { a } else { b };`, `match` expressions, or monadic combinators (`.is_ok_and()`, `.is_some_and()`, `.and_then()`, `.or_else()`, `.unwrap_or_else()`).
     - *Anti-Pattern*: Mutable variables declared before conditional blocks and mutated inside statement branches.
5. **Error Flow & Panic-Free Total Production Code**:
   - Error propagation must flow monadically through the `?` operator.
     - *Target Idiom*: Functions conclude with implicit expression returns (trailing expression without `return`), and errors propagate seamlessly via `?`.
     - *Anti-Pattern*: Manual early `return Err(...)` statements sprinkled across function bodies, or manual `if result.is_err()` matching.
   - **Total Functions (Zero Panics in `src/`)**: Production code in `src/` must be total; every fallible branch must be represented as a strongly typed `Result` or `Option`.
     - *Anti-Pattern*: Calling `.unwrap()` or `.expect()` anywhere in `src/`. `unwrap()` and `expect()` are strictly prohibited in production code (permitted only in test assertions or for statically verifiable constant parsing).
6. **Naming: Semantic Domain Roles over Hungarian Notation**:
   - Identifiers must communicate *domain role, semantic intent, origin, or lifecycle state*.
     - *Target Idiom*: Expressive, unambiguous domain terms (`raw_template_xml`, `source_depot_path`, `overlay_disk_path`, `active_device`, `instance_configuration`).
     - *Anti-Pattern*: Technical type encoding in identifiers (`_str`, `_string`, `_os_str`, `_vec`, `_map`, `_bool`, `_element`, `_parts`).
     - *Anti-Pattern*: Cryptic, truncated names or single-letter identifiers (`|c|`, `|x|`, `|e|`, `p`, `img`). Single-letter variables are strictly prohibited everywhere, including closure arguments.
7. **Parameter Types & Pure vs. Effectful Iterator Semantics**:
   - **Borrow Slices, Own Values**: Functions performing read-only inspection must accept borrowed slices (`&str`, `&Path`, `&[T]`), whereas constructors, builders, and state records consume owned values by move (`String`, `PathBuf`, `Vec<T>`). Never accept `&String`, `&PathBuf`, or `&Vec<T>` as function arguments.
   - **Pure vs. Effectful Iterator Semantics**: Use `.map()`, `.filter()`, and `.fold()` exclusively for pure data projections with zero side-effects. Use `.try_for_each()` or `.for_each()` exclusively when driving linear side-effects. Never use `.map()` to perform mutations or side-effects.

#### Pillar III: Rigorous Verification & Engineering Protocol
1. **The 4-Tier Testing Taxonomy**:
   - **Tier 1: Pure Core Unit Tests**: Validates pure domain logic, AST parsing, tag resolving, diff calculation, and validation logic without any I/O or mocks.
   - **Tier 2: Mocked Contract Tests**: Validates imperative shell workflows (applier, destroyer, planner) against hermetic in-memory mock traits (`MockHypervisor`, `MockStorageManager`).
   - **Tier 3: Real Toolchain Integration Tests**: Validates real CLI invocations (`qemu-img` info, create, rebase, convert, check) against real temporary disk files.
   - **Tier 4: Pipeline Integration Tests**: Validates end-to-end integration across all modules (`ManifestLoader` -> `ContentAddressedImageResolver` -> `DomainTemplateEngine` -> `DomainLifecyclePlanner` -> `DomainLifecycleApplier` -> `DomainLifecycleDestroyer`).
2. **Hermetic Nix Tooling Protocol**:
   - Always execute cargo and development tools via Nix shells:
     - Check & Lint: `nix shell nixpkgs#cargo --command cargo check`
     - Tests: `nix shell nixpkgs#cargo nixpkgs#gcc nixpkgs#qemu-utils --command cargo test`
     - Clippy: `nix shell nixpkgs#cargo nixpkgs#clippy --command cargo clippy --all-targets -- -D warnings`
     - Package Build: `git add packages/onehost && nix build .#packages.x86_64-linux.onehost --no-link`
3. **Engineering Alignment & Quality Gates**:
   - **Context Integrity & The Fresh Read Protocol**:
     - *Mandatory Full Standards Ingestion*: At the beginning of every phase or sub-phase (and whenever context is refreshed or resumed after compaction), the agent MUST perform a fresh read of the entire `## Mandatory Design Guidelines & Engineering Standards` section in whole together (the North Star Architectural Vision, Pillar I, Pillar II, and Pillar III). Skipping, skimming, or relying on partial truncated memory of these engineering standards is strictly prohibited; the entire section must be ingested in full.
     - *Phase Context Bundle*: Simultaneously with the full standards section, the fresh read MUST be accompanied by all other information needed for the active phase:
       1. *Target Phase Blueprint (`task_plan.md`)*: The exact phase definition, task checklist, scope, acceptance criteria, and specific constraints.
       2. *Recent Execution History & State (`progress.md`)*: Current status, recent milestones achieved, key architectural decisions, and error resolutions.
       3. *Domain Contracts & Invariants (`findings.md`)*: Applicable data schemas, domain XML template specifications, state machine transitions, trait interfaces, and verification invariants.
     - *Compaction Defense via AGENTS.md*: `AGENTS.md` at the project root is automatically discovered and loaded into the active system context on every turn by Antigravity. Because workspace rules are part of the active system context, they survive context compaction unconditionally, providing an unalterable defense that enforces the Fresh Read Protocol and Universal Code Equivalence across all future sessions.
   - **Universal Code Equivalence (Zero Second-Class Code)**:
     - *The Absolute Equality Invariant*: NEVER assume code to be less critical or important to any task. Every single line of code—whether pure domain models, imperative I/O shells, low-level streaming parsers, mock implementations, CLI drivers, or test suites—must be held to the exact same high quality standards during any task without exception.
     - *Anti-Rationalization Guardrail*: It is strictly forbidden to bypass, dilute, or excuse engineering standards (such as imperative loops, mutable accumulators, panics via `unwrap`/`expect`, truncated single-letter names, unhandled error cases, or incomplete reporting) by rationalizing code as "just boilerplate", "just low-level reader logic", "just a test mock", "just an internal helper", or "less critical". If code exists in the repository, it demands production-grade excellence.
   - **Zero Unspecified Assumptions**: Never guess or assume unspecified behavior; conduct an interactive interview whenever implementation semantics are ambiguous.
   - **Stage-Gated Senior Code Review**: Perform a thorough senior-level code review upon completion of each phase before advancing.
   - **Have Fun**: Maintain high engineering standards and enjoy the craft.
4. **Continuous Upstream Synchronization & Atomic Commits**:
   - **Zero Local Change Accumulation**: Never allow uncommitted or unpushed local changes to accumulate across development phases.
   - **Atomic Commit & Push on Gate Sign-Off**: Upon completing each phase or sub-phase, passing all verification tiers (check, test, clippy, nix build), and receiving stage-gate approval, immediately commit all related code, test, and planning files with an informative conventional commit message (`feat(...)`, `refactor(...)`, `docs(...)`) and push to upstream (`git push origin <branch>`).
   - **Clean Working Tree**: Ensure the working directory remains clean between phases, preserving a pristine, linear git history synchronized with the remote repository.

---

## Architectural Principles & System Design

```
+-------------------------------------------------------------+
|                         Nix Flake                           |
|  - instances.nix (Declarative VM configurations)            |
|  - Domain XML templates with <disk onehost:role='os-disk'/> |
|  - Flavor OEMDRV derivations (/nix/store/...-oemdrv-*)      |
|  - onehost.json manifest (instances, template_xml, image,   |
|    flavors mapping to OEMDRV paths & hashes, storage dirs)  |
|  - Nix flake apps (onehost plan/apply/backup/destroy)        |
+------------------------------+------------------------------+
                               |
                               v
+-------------------------------------------------------------+
|                      onehost CLI (Rust)                     |
|                                                             |
|  [Content-Addressed Flavor & Image Resolver]                |
|  - Parses image spec: <os>/<version>/<flavor>               |
|  - Maps <flavor> -> OEMDRV Nix store path & hash           |
|  - Resolves golden master: <os>-<ver>-<flavor>-<hash>.qcow2 |
|  - Manages local base cache & instance CoW overlay          |
|                                                             |
|  [Domain Template Engine with <onehost> Namespace]          |
|  - Ingests Nix Domain Template XML                          |
|  - Locates explicit <disk onehost:role='os-disk'>           |
|  - Injects CoW overlay source file & qcow2 driver           |
|  - Injects instance <name>, <uuid>, and <nvram>             |
|                                                             |
|  [Reconciliation & Lifecycle Engine]                        |
|  - Compares concrete XML with live `virsh dumpxml`          |
|  - Compares instance overlay backing file against OEMDRV    |
|  - Defines/updates domain via `virsh define`                |
|  - Manages zero-downtime thin backups & JIT image builds    |
+------------------------------+------------------------------+
                               |
        +----------------------+----------------------+
        |                      |                      |
        v                      v                      v
+---------------+      +---------------+      +---------------+
|HypervisorTrait|      | StorageTrait  |      |ProcessRunner  |
|  (virsh CLI)  |      | (qemu-img CLI)|      |  (swtpm/qemu) |
+-------+-------+      +-------+-------+      +-------+-------+
        |                      |                      |
 (Production)           (Production)           (Production)
        v                      v                      v
+---------------+      +---------------+      +---------------+
|Libvirt Daemon |      |Local Pool &   |      |Host OS &      |
|qemu:///system |      |Depot Store    |      |Hardware       |
+---------------+      +---------------+      +---------------+
```

---

## Key Architectural Decisions

### 1. Explicit OS Disk Identification via Idiomatic `xmlns:onehost`
To clearly communicate intent and avoid guessing which disk in a multi-disk VM should receive the derived OS image, the Domain Template XML declares the custom `onehost` XML namespace and tags the target OS disk using `onehost:role="os-disk"`:

```xml
<domain type='kvm' xmlns:onehost='https://middle-earth.internal/onehost'>
  <name>TEMPLATE</name>
  ...
  <devices>
    <!-- Explicitly designated OS disk managed by onehost -->
    <disk type='file' device='disk' onehost:role='os-disk'>
      <target dev='sda' bus='sata'/>
    </disk>

    <!-- Additional user disks remain 100% untouched by onehost -->
    <disk type='file' device='disk'>
      <source file='/data/games/data.qcow2'/>
      <target dev='sdb' bus='virtio'/>
    </disk>
  </devices>
</domain>
```

#### Key Properties:
- **Idiomatic XML Design**: Uses the standard W3C XML Namespace declaration (`xmlns:onehost='https://middle-earth.internal/onehost'`) alongside existing namespaces like `xmlns:qemu='http://libvirt.org/schemas/domain/qemu/1.0'`.
- **Zero Imported Files**: As an XML namespace identifier, the URI is purely an in-memory disambiguation string. No external `.xsd` or schema files are downloaded or imported.
- **Zero Ambiguity**: If 0 disks or >1 disks are tagged with `onehost:role='os-disk'`, `onehost` fails validation immediately with a clear error.
- **Template-Owned Controller**: The bus type (`sata`, `virtio`, `scsi`) and device name (`sda`, `vda`) are declared directly in the XML template.
- **Sanitization Before Libvirt Registration**:
  1. `onehost` matches `<disk ... onehost:role='os-disk'>`.
  2. It strips `onehost:role='os-disk'` and the root `xmlns:onehost` attribute.
  3. It injects:
     - `<driver name='qemu' type='qcow2'/>`
     - `<source file='/var/lib/libvirt/images/<name>.qcow2'/>`
  4. The concrete XML passed to `virsh define` is 100% standard Libvirt XML conforming strictly to Libvirt's Relax-NG schema.

---

### 2. Content-Addressed Flavor Hashes (Eliminating Manual Revisions)
Instead of manual string revision tags (e.g. `v1`, `v2`, `v3`):
* **Flavor = OEMDRV Derivation in Nix**:
  Each base image flavor (e.g. `looking-glass`, `minimal`, `gaming`) is defined in the Nix flake as an OEMDRV derivation containing the answer files (`autounattend.xml`, `sysprep.xml`), PowerShell scripts (`provision.ps1`), and required tools/installers.
* **Content-Addressed Revision**:
  Nix derivations have cryptographic hashes (e.g. `/nix/store/ba6eafb71234...-oemdrv-looking-glass`). The hash of the OEMDRV derivation *is* the revision!
* **Tag Format**:
  Instances declare images as: `<os>/<build_version>/<flavor>`:
  `win11/26300.9457.pro.en-us/looking-glass`
* **Artifact Naming in Depot Store**:
  `${os}-${build_version}-${flavor}-${oemdrv_hash_short}.qcow2`
  Example: `win11-26300.9457.pro.en-us-looking-glass-ba6eafb7.qcow2`
* **Automatic Cache Invalidation & Drift Detection**:
  When the user updates any provisioning script or installer in Nix:
  1. Nix produces a new OEMDRV store path with a new hash.
  2. `onehost plan` compares the instance's active disk backing file hash against the current OEMDRV hash declared in the manifest.
  3. If they differ, `onehost plan` reports that the base image is out of date and needs a rebuild/rebase.
  4. `onehost image build` uses the hermetic OEMDRV derivation directly to build the golden master.

---

## Declarative Manifest Schema (`onehost.json`)

Nix compiles the lifecycle manifest to JSON via `builtins.toJSON`:

```json
{
  "$schema": "https://middle-earth.internal/schemas/onehost.v1.json",
  "version": "1.0",
  "storage": {
    "depot_store_dir": "/data/depot/virtualization/libvirt/store",
    "depot_iso_dir": "/data/depot/virtualization/libvirt/iso",
    "depot_backup_dir": "/data/depot/virtualization/libvirt/backup",
    "local_pool_dir": "/var/lib/libvirt/images",
    "nvram_dir": "/var/lib/libvirt/qemu/nvram",
    "nvram_template": "/run/libvirt/nix-ovmf/edk2-i386-vars.fd",
    "ovmf_code": "/run/libvirt/nix-ovmf/edk2-x86_64-secure-code.fd"
  },
  "flavors": {
    "looking-glass": {
      "oemdrv_path": "/nix/store/ba6eafb71234...-oemdrv-looking-glass",
      "hash": "ba6eafb7"
    },
    "minimal": {
      "oemdrv_path": "/nix/store/c9d0e1f23456...-oemdrv-minimal",
      "hash": "c9d0e1f2"
    }
  },
  "instances": {
    "win11-gollum": {
      "uuid": "e5a7d620-8931-4bf6-98ec-7e44a30e8c45",
      "template_xml": "/nix/store/7q8w9e...-win11-template.xml",
      "image": "win11/26300.9457.pro.en-us/looking-glass",
      "pool": "default",
      "autostart": false,
      "lifecycle": {
        "on_image_change": "protect",
        "prevent_destroy": false
      }
    },
    "win11-beruthiel": {
      "uuid": "b2c81f24-4b38-4c7f-a0c7-cae44c33b063",
      "template_xml": "/nix/store/a1b2c3...-win11-template.xml",
      "image": "win11/26300.9457.pro.ja-jp/looking-glass",
      "pool": "default",
      "autostart": false,
      "lifecycle": {
        "on_image_change": "recreate",
        "prevent_destroy": false
      }
    }
  }
}
```

---

## Rust Project Architecture (`packages/onehost`)

```
packages/onehost/
├── Cargo.toml
├── Cargo.lock
├── default.nix
├── src/
│   ├── main.rs                   # Entry point and CLI dispatch
│   ├── cli.rs                    # Clap CLI definition (plan, apply, destroy, backup, image, status)
│   ├── xdg.rs                    # XDG Base Directory resolver
│   ├── config/
│   │   ├── mod.rs                # Configuration module exports
│   │   ├── model.rs              # Strongly-typed serde models (StorageConfig, FlavorConfig, InstanceConfig)
│   │   ├── validation.rs         # Zero-default validation rules
│   │   └── loader.rs             # JSON loading and XDG path resolution
│   ├── image/
│   │   ├── mod.rs
│   │   ├── tag.rs                # Image tag parsing (<os>/<version>/<flavor>) & hash key resolution
│   │   └── builder.rs            # Headless QEMU / swtpm builder using OEMDRV from Nix store
│   ├── domain/
│   │   ├── mod.rs
│   │   ├── template.rs           # Locates <disk onehost:role='os-disk'>, injects name, uuid, NVRAM, CoW disk
│   │   └── diff.rs               # XML normalization & diffing against live `virsh dumpxml`
│   ├── hypervisor/
│   │   ├── mod.rs
│   │   ├── traits.rs             # Hypervisor abstraction trait
│   │   ├── virsh.rs              # Production Virsh implementation
│   │   └── mock.rs               # In-memory test mock for TDD
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── traits.rs             # StorageManager abstraction trait
│   │   ├── qemu_img.rs           # Production qemu-img implementation
│   │   └── mock.rs               # In-memory storage mock for TDD
│   ├── lifecycle/
│   │   ├── mod.rs
│   │   ├── planner.rs            # Diff and execution plan generator (Create, Update, Delete, NoOp)
│   │   ├── applier.rs            # Plan executor (base image sync, CoW creation, NVRAM init, virsh define)
│   │   └── destroyer.rs          # Deprovisioning logic (shutdown, virsh undefine, overlay removal)
│   ├── backup/
│   │   ├── mod.rs
│   │   └── engine.rs             # Live snapshot & thin backup engine
│   └── util.rs                   # Process running helpers
└── tests/
    ├── config_tests.rs           # Strict validation & parsing tests
    ├── image_tag_tests.rs        # Tag parsing, OEMDRV hash mapping, artifact key tests
    ├── template_injection_tests.rs # Template XML with <onehost:role='os-disk'> tests
    ├── domain_diff_tests.rs      # Domain XML normalization and diffing tests
    ├── planner_tests.rs          # State diffing tests
    ├── lifecycle_tests.rs        # Mock-driven apply and destroy tests
    └── backup_tests.rs           # Live snapshot sequence tests
```

---

## Storage Pool Architecture & Integration

### The Problem
In Libvirt, storage is organized into named Storage Pools (`virsh pool-list`). Disks should not be placed into arbitrary unmanaged filesystem paths if Libvirt manages them through pools. Furthermore, Gandalf already defines pools declaratively in `middle-earth/hosts/gandalf/virtualization/kvm.nix`:
- `default` -> `/var/lib/libvirt/images` (fast NVMe root)
- `data-legacy` -> `/data/kvm/libvirt/images` (persistent data disk)

### Design Evaluation: Declarative Pool Specification vs Dedicated Pool

| Approach | Behavior | Pros | Cons |
|---|---|---|---|
| **Option A: Declarative Pool Reference (Recommended)** | User specifies the pool name in `onehost.json` (e.g. `"pool": "default"` or `"pool": "onehost"`). `onehost` queries Libvirt (`virsh pool-dumpxml`) to dynamically resolve the pool's physical path. | • 100% declarative: Flake controls pool names and physical disk targets.<br/>• Dynamically adapts if pool path moves in NixOS config.<br/>• Supports multi-tier pools (e.g. NVMe for fast VMs, HDD for bulk storage).<br/>• Respects NixVirt pool definitions. | User must ensure the referenced pool exists in Libvirt (or let `onehost` activate it). |
| **Option B: Dedicated Auto-Created Pool** | `onehost` hardcodes or auto-creates a dedicated pool named `onehost` at a fixed path (e.g. `/var/lib/libvirt/onehost`). | • Isolated volume namespace (`virsh vol-list onehost`). | • Inflexible: cannot choose between NVMe and HDD.<br/>• Creates out-of-band state outside NixVirt control.<br/>• Violates declarative principles. |

### Architectural Recommendation: Declarative Pool Resolution with Auto-Refresh
1. In `onehost.json`, the manifest specifies the storage pool:
   - Global default pool: `"storage": { "pool": "default", ... }`
   - Optional per-instance override: `"instances": { "win11-gollum": { "pool": "fast-nvme", ... } }`
2. At runtime, `onehost`:
   - Inspects the pool in Libvirt via `virsh pool-info <name>`.
   - If active, extracts the physical target path via `virsh pool-dumpxml <name>` (e.g. `/var/lib/libvirt/images`).
   - Uses this path for the local base cache and instance CoW overlays.
   - Executes `virsh pool-refresh <name>` after creating or destroying volumes to keep Libvirt's volume inventory synchronized.

---

## VM Lifecycle & Supported Operations Specification

### 1. VM Lifecycle State Machine

```
                      [ Non-Existent ]
                             │
                             │ onehost image build (if master missing)
                             ▼
                    [ GoldenMasterReady ]
                             │
                             │ onehost apply (creates CoW overlay, copies NVRAM, virsh define)
                             ▼
                       [ Defined/Shutoff ] ◄───────────────┐
                             │                             │
                             │ virsh start                 │ virsh shutdown / destroy
                             ▼                             │
                         [ Running ] ──────────────────────┘
                             │
                             │ onehost backup (online)
                             ▼
                    [ Snapshotting (Live) ]
                    (snap overlay attached)
                             │
                             │ qemu-img convert -c (thin extract)
                             │ virsh blockcommit --active --pivot
                             ▼
                         [ Running ]
                             │
                             │ onehost destroy
                             ▼
                      [ Non-Existent ]
              (CoW overlay deleted, domain undefined)
```

---

### 2. Operations Breakdown

#### Operation A: `onehost plan`
* **Purpose**: Inspect the live environment (Libvirt daemon and storage pool directory) and calculate the difference between the declared state (`onehost.json`) and the actual system state without making any modifications.
* **Inputs & Pre-conditions**:
  - `--config <path>` (or `$XDG_CONFIG_HOME/onehost/onehost.json`).
  - Active Libvirt connection (`qemu:///system`).
  - Specified storage pools must exist in Libvirt.
* **Step-by-Step Workflow**:
  1. Parse and validate `onehost.json` (zero implicit defaults).
  2. For each declared instance:
     - Resolve the target storage pool directory (via `virsh pool-dumpxml <pool>`).
     - Resolve the expected golden master depot path and local cache path from `<os>/<version>/<flavor>` and OEMDRV hash.
     - Check if golden master exists in depot store (`/data/depot/...`).
     - Check if instance CoW overlay exists (`<pool-dir>/<instance>.qcow2`).
     - Check if NVRAM file exists (`/var/lib/libvirt/qemu/nvram/<instance>_VARS.fd`).
     - Ingest `template_xml`, locate `<disk onehost:role='os-disk'>`, and synthesize the concrete Domain XML.
     - Query Libvirt: `virsh dominfo <instance>` and `virsh dumpxml <instance>`.
     - If domain exists in Libvirt:
       - Inspect live domain's OS disk path from live XML and resolve its current pool.
       - Verify active disks via `virsh domblklist`: if any disk ends in `.snap`, flag `! Warning (Dangling snapshot detected from interrupted backup; auto-recommit required)`.
       - If live disk pool differs from declared target pool:
         mark `~ Relocate Storage Pool (from <live-pool> to <target-pool>)`.
       - Compare live XML against synthesized concrete XML (normalizing runtime volatile attributes like dynamic domain `id`, `<alias>`, auto-allocated PCI bus/slots, and auto-generated MAC addresses).
       - Inspect active overlay backing file (`qemu-img info`) to verify if it backs onto the current OEMDRV hash.
       - If backing hash differs: mark `~ Update (Base Image Outdated: running on <old-hash>, desired <new-hash>)`.
       - If XML differs: mark `~ Update (Domain XML drift)`.
       - If identical: mark `<= Unchanged`.
     - If domain does not exist in Libvirt:
       - Mark `+ Create`.
  3. Detect deleted domains: inspect all defined domains in Libvirt carrying the `onehost` metadata tag. If a domain exists in Libvirt but is no longer declared in `onehost.json`, mark `- Destroy`.
  4. Print structured plan summary and exit.

#### Operation B: `onehost apply`
* **Purpose**: Reconcile the declared infrastructure into actual reality, orchestrating golden masters, CoW overlays, NVRAM files, and Libvirt domain registrations.
* **Inputs & Pre-conditions**:
  - Valid `onehost.json`.
  - Libvirt daemon running with root/libvirtd permissions.
  - Windows ISO available if golden master needs JIT building.
* **Step-by-Step Workflow**:
  1. Run `onehost plan` internally to determine required actions.
  2. For each instance marked for creation or update:
     - **Base Image Preparation**:
       - Verify if the golden master `${os}-${version}-${flavor}-${hash}.qcow2` exists in `depot_store_dir`.
       - If missing, automatically trigger JIT build via `onehost image build` using the flavor's OEMDRV derivation from the Nix store.
       - Verify if the local base cache exists in the target storage pool. If missing, copy master from depot to storage pool with `0444` read-only permissions.
     - **Storage & Overlay Provisioning**:
       - **Case 1: No Existing Overlay**:
         - Create fresh CoW QCOW2 overlay (`qemu-img create -f qcow2 -F qcow2 -b <local-base> <instance-disk>`).
         - Copy `nvram_template` (`edk2-i386-vars.fd`) to `<nvram_dir>/<instance>_VARS.fd`.
       - **Case 2: Existing Overlay with Unchanged Base Image**:
         - Preserve the existing overlay disk intact. (Enables non-destructive updates to domain XML like modifying vCPUs, RAM, or network interfaces).
       - **Case 3: Existing Overlay with Changed Base Image (Forces Replacement)**:
         - *Technical Reality*: A QCOW2 overlay stores cluster deltas mapped to the exact sector layout of its original backing file. Attaching an existing overlay to a newly generated base image causes catastrophic guest filesystem corruption and BSODs.
         - *Behavior*: A base image change forces replacement of the overlay disk (`-/+ Recreate`).
         - *Lifecycle Policy & Guardrail Enforcement*:
           1. **`prevent_destroy` check**: If `lifecycle.prevent_destroy == true`, abort with an error preventing replacement.
           2. **`on_image_change == "protect"`**:
              - In non-interactive environments (CI/scripts): abort with error unless `--allow-recreate` is passed.
              - In interactive TTY: prompt `Instance '<name>' base image changed (forces replacement). Destroy existing overlay and recreate? [y/N]: `.
           3. **`on_image_change == "recreate"`**:
              - Ephemeral/cattle instance: proceeds with replacement (prompting in TTY unless `--auto-approve` or `--allow-recreate` is passed).
         - In `onehost plan`: clearly flags `-/+ Recreate (Forces Replacement: base image changed from <old-hash> to <new-hash>, policy: <policy>)`.
         - In `onehost apply`: once approved, shuts down running domain, undefines with NVRAM cleanup, unlinks invalid overlay, creates fresh CoW overlay backed by the new base image, and re-initializes NVRAM.
       - **Case 4: Existing Overlay with Changed Storage Pool (Safe Relocation)**:
         - *Trigger*: Domain exists with an active overlay in pool A, but declared manifest specifies pool B (and base image hash is unchanged).
         - *Behavior*: Safely relocates the overlay disk across storage pools with zero data loss.
         - *In `onehost plan`*: flags `~ Relocate Storage Pool (from <pool-a> to <pool-b>)`.
         - *In `onehost apply`*:
           1. Ensures VM is shut off (issues graceful ACPI shutdown if running).
           2. Ensures golden master base image is cached in destination pool B (`0444` read-only).
           3. Moves overlay disk from pool A directory to pool B directory (`mv` or copy across filesystems + unlink).
           4. Rebases overlay backing file pointer to point to pool B's base image via `qemu-img rebase -u -b <pool_b_base> <pool_b_overlay>`. (Fast metadata-only update preserving 100% of guest data).
           5. Defines updated domain XML pointing to pool B (`virsh define <concrete_xml>`).
           6. Refreshes both storage pools (`virsh pool-refresh <pool_a>`, `virsh pool-refresh <pool_b>`).
     - **Domain Registration**:
       - Ingest `template_xml`, inject instance name, UUID, NVRAM path, and the OS disk referencing `<instance-disk>`.
       - Strip the `onehost:role='os-disk'` attribute and `xmlns:onehost` root attribute.
       - Execute `virsh define <concrete_xml>`.
       - Configure autostart state if `autostart: true`.
     - **Storage Pool Synchronization**:
       - Execute `virsh pool-refresh <pool>` to register the newly created volume in Libvirt's database.
  3. Exit with success summary.

#### Operation C: `onehost destroy`
* **Purpose**: Safely tear down declared VM instances, removing volatile runtime state while safeguarding immutable golden masters and persistent backups.
* **Inputs & Pre-conditions**:
  - `--config <path>` and optional `--target <instance|all>`.
  - Flags: `--force` (force off if running), `--delete-disk` (delete volatile CoW overlay), `--allow-destroy-protected` (override `prevent_destroy`).
* **Step-by-Step Workflow**:
  1. Check `lifecycle.prevent_destroy`: if `true` and `--allow-destroy-protected` is not passed, abort with an error protecting the instance from accidental deletion.
  2. Check domain state via `virsh dominfo <instance>`.
  3. If VM is running:
     - Issue graceful ACPI shutdown (`virsh shutdown <instance>`).
     - If VM fails to shut down within timeout (or `--force` is set), issue `virsh destroy <instance>` (immediate power cut).
  4. Undefine domain in Libvirt: `virsh undefine <instance> --nvram` (unregisters domain and cleans up instance NVRAM file).
  5. If `--delete-disk` is specified: unlink the volatile instance CoW overlay disk (`<pool-dir>/<instance>.qcow2`).
  6. Immutable golden master images in `depot_store_dir` and backups in `depot_backup_dir` are strictly preserved.
  7. Execute `virsh pool-refresh <pool>`.

#### Operation D: `onehost backup`
* **Purpose**: Create application-consistent or crash-consistent live thin snapshot backups of VM overlays, NVRAM, and XML definitions, staging them to persistent depot storage with a structured `manifest.json` for subsequent cloud backup by Restic.
* **Inputs & Pre-conditions**:
  - Target instance defined in Libvirt.
  - Storage pool directory with read/write access.
  - `depot_backup_dir` path available (`/data/depot/virtualization/libvirt/backup`).
* **Step-by-Step Workflow**:
  1. Identify VM status via `virsh dominfo <instance>`.
  2. Resolve all attached disks and their target devices (e.g. `sda`, `sdb`) via `virsh domblklist <instance> --details`.
  3. Query `qemu-guest-agent` availability: if responsive, set `quiesce = true` (invokes Windows VSS); otherwise log a warning and fall back to crash-consistent snapshotting (`quiesce = false`).
  4. **Case 1: VM is RUNNING (Zero-Downtime Live Backup)**:
     - Prepare atomic snapshot command specifying all attached disks:
       `virsh snapshot-create-as --domain <name> --name backup-<name> --diskspec sda,file=<disk1>.snap --diskspec sdb,file=<disk2>.snap --disk-only --atomic --no-metadata [quiesce flag]`
     - **Arm RAII Cleanup Guard**: A Rust scope-guard is armed immediately. If any subsequent extraction step fails or is interrupted, the guard automatically executes `virsh blockcommit <name> <dev> --active --pivot` for each disk to guarantee no VM is left running on a dangling `.snap` overlay.
     - For each frozen base disk:
       `qemu-img convert -U -O qcow2 -c -B <backing-file> -F qcow2 <base-disk> <staging>/<archive_file>.tmp`
     - Perform live active blockcommit to collapse temporary snapshots back into the base disks:
       `virsh blockcommit <name> <dev> --base <base-disk> --top <snap-disk> --active --pivot`
     - Disarm RAII guard and unlink ephemeral `*.snap` files.
  5. **Case 2: VM is SHUT OFF (Offline Backup)**:
     - Directly convert and compress each thin overlay:
       `qemu-img convert -U -O qcow2 -c -B <backing-file> -F qcow2 <disk> <staging>/<archive_file>.tmp`
  6. Atomically rename `*.tmp` files to final archive filenames.
  7. Copy `<nvram_dir>/<name>_VARS.fd` to `<staging>/nvram.fd` and dump current `virsh dumpxml <name>` to `<staging>/domain.xml`.
  8. Write structured `<staging>/manifest.json` containing:
     - Instance metadata (name, UUID, timestamp).
     - Base image metadata (tag, OEMDRV hash, golden master filename).
     - Disks list (target device, archive file name, virtual and physical byte sizes).
     - Consistency level (`vss_quiesced` or `crash_consistent`).

#### Operation E: `onehost image build`
* **Purpose**: Orchestrate automated headless installation of Windows into an immutable golden master QCOW2 image using a specific flavor's OEMDRV derivation and official ISO.
* **Inputs & Pre-conditions**:
  - Image tag: `<os>/<version>/<flavor>`.
  - OEMDRV derivation path in Nix store (`/nix/store/...-oemdrv-<flavor>`).
  - Windows ISO located in `depot_iso_dir`.
  - `swtpm` (TPM 2.0 emulator) and `qemu-system-x86_64` available in runtime.
  - Ephemeral scratch directory in `$XDG_CACHE_HOME/onehost/build/`.
* **Step-by-Step Workflow**:
  1. Verify if master already exists at `${depot_store_dir}/${os}-${version}-${flavor}-${hash}.qcow2`. If present, exit immediately (immutable store).
  2. Spawn isolated `swtpm` instance in socket mode on `$XDG_RUNTIME_DIR/onehost/swtpm.sock`.
  3. Create an ephemeral 64GB QCOW2 build disk in cache dir.
  4. Launch headless QEMU VM:
     - OVMF UEFI code and fresh NVRAM vars.
     - Attached TPM 2.0 emulator socket.
     - Ephemeral build disk.
     - Windows 11 installation ISO.
     - OEMDRV ISO containing `autounattend.xml`, `provision.ps1`, Looking Glass host, VirtIO drivers.
     - VirtIO guest tools ISO.
     - QEMU monitor unix socket on `$XDG_RUNTIME_DIR/onehost/monitor.sock`.
  5. Send automated keystrokes (`ret`, `spc`) to monitor socket during boot to bypass the "Press any key to boot from CD/DVD" prompt.
  6. Windows unattended setup installs the OS, executes `provision.ps1` on first logon, generalizes the system via Sysprep, and powers off the VM.
  7. Wait for QEMU process exit.
  8. Compress and stage the build disk to `${depot_store_dir}/${master_filename}.tmp` using:
     `qemu-img convert -O qcow2 -c <build-disk> ${depot_store_dir}/${master_filename}.tmp`
  9. **Atomic Master Promotion**:
     - Verify integrity of `${master_filename}.tmp` via `qemu-img check`.
     - Atomically rename `.tmp` to `${master_filename}.qcow2`.
     - Set permissions to `0444` (strictly read-only immutable master).
     - Clean up ephemeral scratch build directories and sockets.

#### Operation F: `onehost status`
* **Purpose**: Query and present real-time operational status across declared instances, storage pools, backing images, and passthrough devices.
* **Outputs**:
  - Domain status: running / shutoff / paused, vCPU count, memory allocation.
  - Storage pool status: capacity, available space, active pool path.
  - Disk overlay status: virtual size, physical size on disk, backing file path, backing hash synchronization status, active snapshot alerts (flags dangling `.snap` files).
  - Hardware devices: Looking Glass KVMFR node status, SR-IOV virtual function allocation.

#### Operation G: `onehost restore <backup-dir>`
* **Purpose**: Single-command disaster recovery to restore an instance from a staged backup directory (retrieved from Restic or local depot storage).
* **Inputs & Pre-conditions**:
  - `<backup-dir>` containing `manifest.json`, `domain.xml`, `nvram.fd`, and compressed disk archives.
* **Step-by-Step Workflow**:
  1. Parse and validate `<backup-dir>/manifest.json`.
  2. Resolve target storage pool directory (via manifest or CLI override).
  3. Verify that the golden master required by `manifest.json` exists in `depot_store_dir` (or build/fetch it if missing).
  4. Ensure base image is cached in the target storage pool (`0444` read-only).
  5. Decompress / copy each disk archive from `<backup-dir>` into the target storage pool.
  6. Rebase the restored overlay's backing file to point to the local pool's base image via `qemu-img rebase -u -b <pool_base> <restored_overlay>`.
  7. Copy `<backup-dir>/nvram.fd` to the host's `<nvram_dir>/<instance>_VARS.fd`.
  8. Register the domain in Libvirt using `virsh define <backup-dir>/domain.xml`.
  9. Refresh storage pool (`virsh pool-refresh <pool>`).
  10. Output success summary indicating the restored instance is defined and ready to start.

---

### Phase 10b Architectural Findings: Idiomatic Functional Builders & Strict Move Semantics

#### 1. The Fallacy of `fn(&self, ...) -> Self` as Call-Site "Convenience"
During the Phase 10b review, we investigated builder patterns for immutable domain models and XML AST nodes (`DomainXmlElement`, `OnehostManifest`, `InstanceConfig`):
- **The Pseudo-Convenience of `fn(&self, ...) -> Self`**:
  - Proponents of this signature argue that it enables callers holding borrowed references (`&T`) to call `.with_x()` without typing `.clone()` explicitly.
  - **The Fatal Flaw**: To produce a new `Self` from an immutable reference `&self`, the method **must clone every single field of `Self`**, even fields completely unrelated to the mutation!
  - In hierarchical AST trees (`DomainXmlElement`), every invocation of `element.with_attribute(...)` or `element.with_child(...)` forces a recursive deep clone of the entire child vector and attribute map.
  - In chained builder calls (`element.with_a().with_b().with_c()`), intermediate rvalues are cloned repeatedly, producing $O(N^2)$ unnecessary heap allocations.
  - It creates hidden cost: the caller believes they are performing a cheap adjustment, while the runtime executes hidden, heavy memory allocations behind an opaque signature.

#### 2. The Idiomatic Functional Pattern: `fn(self, ...) -> Self`
Rust's ownership and move semantics provide zero-cost functional composition when methods consume `self` by value:
- **Move Semantics & Struct Update Syntax**:
  ```rust
  pub fn with_pool(self, pool: Option<String>) -> Self {
      Self {
          pool,
          ..self
      }
  }
  ```
  - Unchanged fields (such as large `template_xml` strings, UUIDs, or subtrees) are moved at the pointer/stack level with **zero heap copies**.
- **Vector and Map Transformations**:
  ```rust
  pub fn with_child(self, child: DomainXmlElement) -> Self {
      let mut children = self.children;
      children.push(child);
      Self {
          children,
          ..self
      }
  }
  ```
  - Existing heap vectors move directly into the new struct.
- **Explicit Cost at the Call Site**:
  - When the caller already owns the value (`let el = DomainXmlElement::new(...).with_child(...);`), chaining incurs **zero clones**.
  - When the caller holds only a borrowed reference `&T` and needs a modified copy, the allocation is made transparent and deliberate at the call site:
    ```rust
    let modified = original.clone().with_pool(Some("target-pool".to_string()));
    ```
  - This honors Rust's foundational principle: **costs must be visible, explicit, and opt-in**.

#### 3. Core Engineering Mandate: Never Sacrifice Idiomatic Design for Convenience
We have enshrined **Rule 9** in our design guidelines:
> **Never prefer call-site convenience or any convenience of any kind over idiomatic code.**

- Under no circumstances should APIs be degraded with hidden allocations, awkward reference signatures, or non-idiomatic workarounds merely to save a few characters at call sites.
- All builders, transformers, and pipeline stages across `onehost` must strictly adhere to by-value functional consumption (`fn(self, ...) -> Self`), returning born-valid values.

---

### Senior-Level Architecture Finding: Preference of Public Attributes over Getter Functions in Functional Domain (Rule 10)

#### 1. The Conflict: OOP Encapsulation vs Functional Algebraic Data Types
In traditional object-oriented programming (e.g. Java, C++, C#), classes hide their state behind `private` fields and expose trivial getter functions (`getX()`, `getY()`) to achieve encapsulation. This ceremony is designed to guard mutable internal state against uncontrolled external mutation.

In Rust's functional paradigm, state and behavior are decoupled. Data structures (AST nodes, configuration manifests, image tag specifications, diff results) are **pure algebraic data records** and **immutable values**. Furthermore, per Rule 8, values are **born valid**—invariants are strictly verified at the point of construction (`new()`, `from_str()`, `serde_json::from_str`). Once created, an instance is an immutable snapshot of valid domain state.

#### 2. The Cost of Getter Functions in Functional Rust
Hiding fields of born-valid data structures behind private attributes with trivial getter functions (`pub fn foo(&self) -> &Foo { &self.foo }`) creates serious engineering friction:
1. **Prevents Pattern Matching & Destructuring**: Callers cannot use native Rust pattern matching or destructuring by move (`let InstanceConfiguration { uuid, template_xml, .. } = instance;`), which is the bedrock of functional Rust.
2. **Prevents Struct Update Syntax**: Callers cannot leverage Rust's zero-copy struct update syntax (`InstanceConfiguration { pool: Some(new_pool), ..instance }`).
3. **Boilerplate Noise**: Hundreds of lines of repetitive getter methods clutter domain models without providing any invariants protection or abstraction.
4. **Forces Awkward API Naming**: Leads to artificial discrepancies between field names and getter methods (e.g. `template_xml` vs `template_xml_path()`, `depot_store_dir` vs `depot_store_directory()`).

#### 3. Core Mandate: Public Attributes for Pure Data Records
Under **Rule 10**, all pure data structures and AST nodes across `onehost` must expose their fields directly as `pub`:
- **`DomainXmlElement`**: `pub tag_name`, `pub attributes`, `pub children`, `pub text_content`.
- **`OnehostManifest`**: `pub schema`, `pub version`, `pub storage`, `pub flavors`, `pub instances`.
- **`StorageDirectoriesConfiguration`**: `pub depot_store_dir`, `pub depot_iso_dir`, `pub depot_backup_dir`, `pub nvram_dir`, `pub nvram_template`, `pub ovmf_code`, `pub default_pool`.
- **`FlavorConfiguration`**: `pub oemdrv_path`, `pub hash`.
- **`InstanceConfiguration`**: `pub uuid`, `pub template_xml`, `pub image`, `pub pool`, `pub autostart`, `pub lifecycle`.
- **`InstanceLifecycleConfiguration`**: `pub on_image_change`, `pub prevent_destroy`.
- **`ImageTagSpecification`**: `pub operating_system`, `pub build_version`, `pub flavor_name`.
- **`FlavorDerivationMetadata`**: `pub oemdrv_nix_store_path`, `pub content_hash`.
- **`DomainXmlDiffResult`**: `pub has_drift`, `pub differences`, `pub normalized_synthesized_xml`, `pub normalized_live_xml`.

Methods on data structures are strictly reserved for:
1. **Construction & Validation**: Invariant enforcement at instantiation (`new()`, `from_str()`).
2. **Domain Operations / Queries**: Non-trivial calculations or fallback hierarchies (e.g. `effective_pool()`, `get_attribute()`, `has_child()`).
3. **Functional Builders & Transformations**: Move-based transformations following `fn(self, ...) -> Self` (e.g. `with_pool()`, `with_child()`, `transform_children()`).

---

### Senior-Level Architecture Finding: Avoid Redundant Type Information in Identifiers (Rule 11)

#### 1. The Code Smell: Hungarian Notation & Type-Leaking Identifiers
A common anti-pattern in codebases transitioning between imperative and functional styles is encoding technical data types into identifier names:
- Suffixing string representations: `file_name_os_str`, `path_string`, `raw_xml_str`, `domain_name_str`.
- Suffixing collection types: `items_vec`, `config_map`, `tags_list`, `instances_hashmap`.
- Suffixing AST types: `child_element`, `target_element`, `path_node`.
- Suffixing primitives: `is_valid_bool`, `timeout_int`, `port_u16`.

#### 2. Why Type-Leaking Identifiers Degrade Code Quality
1. **Redundancy & Noise**: In Rust, the compiler strictly enforces types, and developer tooling (rust-analyzer) instantly displays the exact type of any symbol. Duplicating type names inside variable identifiers creates visual clutter without providing new information.
2. **Brittle Refactoring**: If a representation changes (e.g. from `&str` to `&Path`, or `Vec<T>` to `BTreeSet<T>`), type-leaking variable names either become outright lies or force tedious mechanical renames throughout the codebase.
3. **Symptom of Missing Domain Semantics**: When a variable is named `path_string` or `child_element`, the author named *what the data structure is* in memory, rather than *what role the value plays in the domain logic*.

#### 3. Core Mandate: Semantic Domain Naming
Under **Rule 11**, identifiers must communicate **domain purpose, role, lifecycle state, or origin**:
- Instead of `path_string` -> use `target_directory`, `pool_path`, or `unresolved_path`.
- Instead of `child_element` or `target_element` -> use `child`, `target`, `candidate`, or `enclosed_device`.
- Instead of `items_vec` -> use `entries`, `definitions`, or `components`.
- Instead of `name_os_str` -> use `raw_name` or `unvalidated_name`.
- Instead of `config_map` -> use `configurations` or `flavor_registry`.

---

### Phase 12c.6: Comprehensive Architectural Analysis & Structural Refactoring

#### 1. Foundational Context & Invariant
Per the **Universal Code Equivalence & Absolute Standards** principle:
> **"NEVER assume code to be less critical or important to any task. All code across the entire codebase—whether pure domain logic, imperative execution shell, low-level parser routines, CLI dispatchers, error definitions, mock drivers, test harnesses, or integration suites—must be held to the exact same uncompromising, high quality standards during any task. There are zero second-class components, zero exemptions, and zero quality tiers."**

Every scope of the codebase is treated equally. Both **deep domain enhancements** (semantic domain types, born-valid invariants, AST query/transformation combinators) and **boundary/trait interface cohesion** (trait ergonomics, mock fidelity, core vs. shell decoupling, forward compatibility for Phase 13) are equally critical and important.

#### 2. The 5 Core Architectural Audit Tracks
1. **Module Coupling & Dependency Graph**:
   - Verify strict unidirectional layering: `config` / `domain` (Pure Core) -> `hypervisor` / `storage` (Trait Contracts) -> `lifecycle` (Imperative Shell).
   - Ensure zero inverted dependencies, zero circular references, and strict visibility encapsulation (`pub(crate)` vs. `pub`).
2. **Deep Domain Modeling & Type-Level Invariants**:
   - Eradicate primitive obsession: evaluate semantic types/newtypes for storage pools, device bus models, disk roles, and domain names.
   - Elevate `DomainXmlElement` AST query/transformation ergonomics: add higher-order functional combinators for child lookups, attribute filters, and recursive tree transforms.
   - Enforce 100% "born-valid" data structures across all domain records.
3. **Trait Boundary & Hardware Interface Cohesion**:
   - Audit `trait Hypervisor` and `trait StorageManager` for interface segregation and single responsibility.
   - Eliminate any leaking of filesystem mutations or command execution details into imperative shells.
   - Standardize parameter borrowing (`&str`, `&Path`) and owned return semantics across all trait methods.
4. **Universal Code Equivalence & Mock Fidelity**:
   - Audit `MockHypervisor` and `MockStorageManager` against production implementations (`virsh.rs`, `qemu_img.rs`) to ensure 100% behavioral, error-mode, and state-transition parity.
   - Ensure mock isolation without host environment leaks, and verify that test suites adhere to production-grade coding standards.
5. **Error Architecture & Forward-Compatibility**:
   - Harmonize all 9 error hierarchies across modules into clean categories (Domain, Infrastructure, Policy).
   - Pressure-test traits and structures against Phase 13 requirements (multi-disk atomic snapshotting, active `blockcommit --pivot`, RAII cleanup guards, and thin compressed disaster recovery).

#### 3. The 6 Concrete Architectural Oversights Discovered
1. **Leaky Trait Boundary in Applier (Raw `fs` Operations for NVRAM)**:
   - *Location*: `src/lifecycle/applier.rs:67-75`
   - *Violation*: Imperative shell calls raw `fs::create_dir_all` and `fs::copy` instead of delegating to `StorageManager`. Bypasses trait boundary, leaks to host disk during tests.
   - *Solution*: Add `initialize_nvram` to `StorageManager`, implement in `QemuImgStorage` and `MockStorageManager`, and delegate from `applier.rs`.
2. **Host Filesystem Leaks in `MockStorageManager`**:
   - *Location*: `src/storage/mock.rs:295-349`, `src/storage/traits.rs:132-174`
   - *Violation*: Mock calls `source_depot_path.exists()`, `std::fs::copy()`, `std::fs::rename()`, and `std::fs::remove_file()`, mutating the developer's real machine. Trait provides default implementations with real filesystem side-effects.
   - *Solution*: Remove all `std::fs` operations from `MockStorageManager` to make it 100% hermetic and in-memory. Keep shared helpers in `traits.rs` but remove default implementations from the trait so implementors handle their backends explicitly.
3. **Primitive Obsession in Configuration (`InstanceUuid`)**:
   - *Location*: `src/config/model.rs:99`, `src/config/validation.rs:143-167`
   - *Violation*: `InstanceConfiguration.uuid` is raw `String`. Instantiating via `InstanceConfiguration::new` allows invalid UUIDs, violating the "born-valid" invariant.
   - *Solution*: Introduce born-valid `InstanceUuid` newtype enforcing RFC-4122 on construction.
4. **Repetitive XML AST Traversal in `DomainXmlElement`**:
   - *Location*: `src/domain/template.rs:68-80`, `src/domain/diff.rs:274-320`, `src/hypervisor/mock.rs:85-118`
   - *Violation*: Deep matching and chaining boilerplate repeated across multiple modules (`find_child_by_tag(...).and_then(...).and_then(...)`).
   - *Solution*: Add higher-order query combinators to `DomainXmlElement` (`child_attribute`, `child_text`, `path_text`, `find_path`, `find_children`, `has_attribute_value`).
5. **Stringly-Typed Errors & Unhandled Error Swallowing**:
   - *Location*: `src/lifecycle/planner.rs:155-160`, `src/lifecycle/destroyer.rs:40-75`
   - *Violation*: Emitting `LifecycleError::ConfigurationError` strings instead of strongly-typed error variants. Line 75 in `destroyer.rs` discards `undefine_domain` results with `let _ =`, silencing fatal hypervisor failures.
   - *Solution*: Add `InstanceNotDeclared` and `ManifestValidationError` to `LifecycleError`. Match `undefine_domain` explicitly, ignoring only `DomainNotFound` and propagating true infrastructure errors.
6. **Forward-Compatibility for Phase 13: `restore_thin_backup` in `StorageManager`**:
   - *Location*: `src/storage/traits.rs:100-175`
   - *Need*: Phase 13 disaster recovery restore needs a symmetrical `restore_thin_backup` method complementary to `convert_thin_backup`.
   - *Solution*: Add `restore_thin_backup` to `StorageManager`, implement in `QemuImgStorage` and `MockStorageManager`, and verify with unit tests.

#### 4. Technical Invariants & Signatures
```rust
// 1. Born-valid UUID
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstanceUuid(String);
impl InstanceUuid {
    pub fn parse(raw_uuid: &str) -> Result<Self, ManifestValidationError>;
    pub fn as_str(&self) -> &str;
}

// 2. High-level DomainXmlElement combinators
impl DomainXmlElement {
    pub fn child_attribute(&self, child_tag: &str, attribute_key: &str) -> Option<&str>;
    pub fn child_text(&self, child_tag: &str) -> Option<&str>;
    pub fn find_path(&self, tag_path: &[&str]) -> Option<&DomainXmlElement>;
    pub fn path_text(&self, tag_path: &[&str]) -> Option<&str>;
    pub fn find_children<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a DomainXmlElement>;
    pub fn has_attribute_value(&self, key: &str, value: &str) -> bool;
    pub fn has_matching_attribute(&self, predicate: impl Fn(&str, &str) -> bool) -> bool;
}

// 3. StorageManager additions
pub trait StorageManager: Send + Sync {
    // ... existing methods ...
    fn initialize_nvram(&self, template_path: &Path, destination_nvram_path: &Path) -> Result<(), StorageError>;
    fn restore_thin_backup(&self, archive_path: &Path, destination_overlay_path: &Path, backing_file_path: Option<&Path>) -> Result<(), StorageError>;
}

// 4. LifecycleError additions
pub enum LifecycleError {
    // ...
    #[error("Instance '{instance_name}' is not declared in the manifest")]
    InstanceNotDeclared { instance_name: String },
    #[error("Manifest validation error: {0}")]
    ManifestValidationError(#[from] ManifestValidationError),
}
```

#### 5. Architectural Lessons Learned & Compaction Defense Architecture

##### 1. Compaction Defense via Workspace Rules (`AGENTS.md`)
- **Mechanism**: Placed at `/home/tarberd/middle-earth/AGENTS.md`, this file is automatically ingested into Antigravity's active system/workspace prompt on every turn.
- **Compaction Immunity**: Because workspace rules are part of the active system context and are not part of the conversational trajectory, they **unconditionally survive context compaction**.
- **Fresh Read Protocol Invariant**: Codified in `AGENTS.md`, whenever `<CONTEXT_SUMMARY>` is detected in the prompt, the agent is bound by rule to perform a fresh read of the design guidelines, phase blueprint, and domain contracts before taking action.

##### 2. Trait Purity & Elimination of Leaky Defaults
- **Lesson**: Default trait implementations that perform real host I/O (e.g., `traits.rs` calling `std::fs::copy` or `std::fs::rename`) silently bleed into test mocks (`MockStorageManager`).
- **Standard**: Traits must never embed default implementations with real host filesystem side-effects. All I/O must reside explicitly within production implementors (`QemuImgStorage`).

##### 3. Imperative Shell Boundary Integrity
- **Lesson**: The orchestration shell (`applier.rs`, `destroyer.rs`, `planner.rs`) must strictly coordinate actions across traits without directly mutating the host environment.
- **Standard**: Zero raw `std::fs` calls in the imperative shell. Operations like NVRAM directory creation and template file copying belong behind `StorageManager::initialize_nvram`.

##### 4. Hermetic Mocks (100% In-Memory Isolation)
- **Lesson**: Mocks that check `Path::exists()` or touch the host disk break the contract of hermetic unit/contract testing and can alter developer disk state.
- **Standard**: Mocks must track state purely in-memory (e.g., virtual file paths, recorded action queues) and maintain 100% behavioral parity with production drivers without touching the real disk.

##### 5. Error Hierarchy Harmonization & Zero Silent Swallowing
- **Lesson**: Catch-all string errors (`ConfigurationError { details }`) degrade error handling, and silent discards (`let _ = undefine_domain(...)`) mask real infrastructure breakdowns.
- **Standard**: Use typed domain variants (`InstanceNotDeclared`, `ManifestValidationError`), and never swallow results with `let _ =`; explicitly match expected idempotent variants (e.g. `DomainNotFound => Ok(())`) while bubbling true errors.

##### 6. Born-Valid Domain Types
- **Lesson**: Storing domain identifiers as raw `String` permits unvalidated instantiation, violating the parse-don't-validate principle.
- **Standard**: Wrap identifiers in dedicated newtypes (`InstanceUuid`) enforcing strict syntactic invariants (e.g. RFC-4122) upon construction.

---

### Phase 13: Stage 8 - Thin Backup & Restore Engine Architecture

#### 1. System Contracts & Module Architecture
- **`src/backup/model.rs`**:
  - `BackupConsistencyLevel`: `VssQuiesced`, `CrashConsistent`, `Offline`.
  - `DiskBackupEntry`: `target_device`, `archive_filename`, `virtual_size_bytes`, `archive_size_bytes`, `backing_file`.
  - `BackupManifest`: `instance_name`, `instance_uuid`, `timestamp`, `consistency_level`, `storage_pool`, `base_image_tag`, `base_image_hash`, `golden_master_filename`, `disks`, `domain_xml_filename`, `nvram_filename`.
  - `BackupOptions`: `quiesce: bool` (default true), `compress: bool` (default true), `timestamp: Option<String>`.
  - `RestoreOptions`: `target_pool: Option<String>`, `depot_store_dir: Option<PathBuf>`, `nvram_dir: Option<PathBuf>`, `allow_overwrite: bool`.
- **`src/backup/guard.rs`**:
  - `ActiveSnapshotInfo`: `target_device: String`, `base_path: PathBuf`, `snapshot_path: PathBuf`.
  - `SnapshotCleanupGuard<'a, H: Hypervisor>`: RAII scope guard tracking active `.snap` files. On drop while armed, invokes `hypervisor.blockcommit(..., active: true, pivot: true)` to guarantee zero dangling snapshots on failure, cancellation, or panic.
- **`src/backup/engine.rs`**:
  - `BackupEngine<'a, H: Hypervisor, S: StorageManager>`: Orchestrates online/offline thin backup pipelines.
- **`src/backup/restore.rs`**:
  - `RestoreEngine<'a, H: Hypervisor, S: StorageManager>`: Executes single-command disaster recovery.
- **`src/backup/mod.rs`**:
  - Strongly typed `BackupError` and `RestoreError` hierarchies (0 stringly-typed errors).

#### 2. Online Live Backup & RAII Rollback Guarantee
1. Domain state checked via `hypervisor.domain_info`.
2. All attached disks resolved via `hypervisor.list_block_devices`.
3. Multi-disk atomic snapshot requested with `DiskSnapshotSpecification` for each disk simultaneously.
4. Opportunistic quiescing: tries `quiesce: true` (VSS); if hypervisor returns error indicating guest agent failure or unresponsive VSS, catches error, logs warning, and retries with `quiesce: false` (crash-consistent).
5. Arm `SnapshotCleanupGuard`.
6. Extract and compress thin delta overlays via `storage.convert_thin_backup` to `.tmp` files and atomic move via `storage.move_file_safely`.
7. Recommit live snapshots via `hypervisor.blockcommit(..., active: true, pivot: true)` and delete temporary `.snap` files via `storage.delete_image`.
8. Disarm `SnapshotCleanupGuard`.
9. Back up NVRAM via `storage.initialize_nvram`, dump Domain XML via `hypervisor.dump_xml`, and write `manifest.json`.

#### 3. Disaster Recovery Restoration Pipeline
1. Parse and validate `<backup-dir>/manifest.json`.
2. Guardrail check: if domain already exists and `!allow_overwrite`, abort with `RestoreError::DomainAlreadyExists`.
3. Resolve target storage pool directory path via `hypervisor.resolve_pool_path`.
4. Ensure base image is cached in target pool: if not cached, verify depot store and copy via `storage.copy_base_image` (`0444`).
5. For each disk entry in manifest: execute `storage.restore_thin_backup` to reconstruct thin CoW overlay reattached to the base backing file.
6. Ingest domain XML and restore NVRAM to host destination via `storage.initialize_nvram`.
7. Register domain in Libvirt via `hypervisor.define_domain`.
8. Refresh storage pool in Libvirt via `hypervisor.pool_refresh`.

#### 4. Trait Purity & Hermetic Mock Isolation
- Added `write_file(&self, destination_path: &Path, content: &str)` and `read_file(&self, source_path: &Path)` to `StorageManager` trait.
- Production `QemuImgStorage` delegates to `std::fs`.
- `MockStorageManager` tracks virtual files in `files: HashMap<PathBuf, String>`, achieving 100% in-memory hermeticity with zero host disk operations.

---

### Phase 13b: Full Disaster Recovery & Backup Integration Suite

#### 1. End-to-End Declarative Round-Trip Lifecycle
- **The Lifecycle Invariant**:
  `plan(restore(backup(apply(plan(declared_state))))) == InstancePlanAction::NoOp`
  Applying a declarative manifest creates the concrete hypervisor domain and thin CoW storage overlays. Backing up the instance captures domain XML, NVRAM, thin compressed deltas, and metadata manifest. Simulating disaster by completely destroying the instance (`--delete-disk`) and undefining it from Libvirt leaves the hypervisor in a state requiring `Create`. Restoring from the backup directory reconstructs overlays reattached to the golden master backing file, restores NVRAM, defines the domain, and refreshes Libvirt storage pools. Calling `DomainLifecyclePlanner::plan` against the restored instance produces `InstancePlanAction::NoOp` with zero drift (`diff.has_drift == false`).

#### 2. Automatic Local Base Image Cache Repopulation
- In a disaster scenario where the local storage pool is wiped clean (loss of filesystem or node migration), the local pool cache of the golden master (`${os}-${version}-${flavor}-${hash}.qcow2`) is destroyed along with instance overlays.
- `RestoreEngine::restore_instance` checks if the base image is present in the target pool directory. If missing, it automatically resolves `depot_store_dir` from the options or root manifest, copies the golden master from the depot into the local pool cache with `0444` read-only permissions via `storage.copy_base_image`, and attaches the restored thin overlay to this newly repopulated pool cache.

#### 3. Multi-Disk Disaster Recovery Invariance
- Multi-disk VM architectures (`sda` OS disk + `sdb` secondary data disk):
  - `DomainTemplateEngine` designates exactly one OS disk (`onehost:role='os-disk'`), synthesizing its source overlay path to `<pool>/<instance>.qcow2`.
  - Secondary disks declared in the template `<devices>` block are preserved with their explicit target devices and source paths (e.g. `<pool>/<instance>-sdb.qcow2`).
  - `BackupEngine` queries `hypervisor.list_block_devices`, capturing thin backup entries for both `sda` and `sdb` in `manifest.json`.
  - `RestoreEngine` reconstructs overlays for every disk in `manifest.disks` (`sda` mapped to `<instance>.qcow2`, secondary disks mapped to `<instance>-<target_device>.qcow2`), restoring NVRAM, defining the multi-disk domain, and achieving zero drift upon subsequent planning.

#### 4. Real CLI Toolchain Verification (`qemu-img convert -c`)
- Real `QemuImgStorage` executing real `qemu-img` binary under temporary isolation validates that:
  - `convert_thin_backup` with `compress: true` creates thin zlib-compressed QCOW2 deltas substantially smaller than virtual disk capacity.
  - `restore_thin_backup` reconstructs the QCOW2 overlay with proper backing file headers.
  - `check_image` validates 0 internal cluster corruption.
  - All destination parent directories are automatically created (`std::fs::create_dir_all`) preserving the Shell Trait Boundary Invariant (0 `std::fs` calls in imperative shells).




