# Task Plan: Refactor Windows VM Provisioning to Onehost

## Goal
Refactor the Nix Flake Windows VM provisioning architecture to rely exclusively on the newly created `onehost` CLI tool, eliminating over 730 lines of dead bash scripts, obsolete domain XML generators, legacy workarounds, and deprecated backward-compatibility shims.

## Next Step
Project complete! All 6 phases successfully executed, verified, and delivered.

## Current Phase
Phase 6: Upstream Synchronization & Delivery (Complete)

## Mandatory Design Guidelines & Engineering Standards

### North Star Architectural Vision
> **"A Functional Core with an Imperative Shell, orchestrating declarative, idempotent, and hermetic Windows Libvirt/KVM lifecycles via OpenTofu-style reconciliation."**

### Foundational Principle: Universal Code Equivalence & Absolute Standards
> **"NEVER assume code to be less critical or important to any task. All code across the entire codebase—whether pure domain logic, imperative execution shell, low-level parser routines, CLI dispatchers, error definitions, mock drivers, test harnesses, or integration suites—must be held to the exact same uncompromising, high quality standards during any task. There are zero second-class components, zero exemptions, and zero quality tiers."**

### Pillar I: Architectural Blueprint & System Contracts
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

### Pillar II: Functional Rust Craft & Idiomatic Implementation
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

### Pillar III: Rigorous Verification & Engineering Protocol
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
     - *Mandatory Full Standards Ingestion*: At the beginning of every phase or sub-phase (and whenever context is refreshed or resumed after compaction), the agent MUST perform a fresh read of the entire `## Mandatory Design Guidelines & Engineering Standards` section in whole together (the North Star Architectural Vision, Pillar I, Pillar II, and Pillar III).
     - *Phase Context Bundle*: Target Phase Blueprint (`task_plan.md`), Recent Execution History & State (`progress.md`), Domain Contracts & Invariants (`findings.md`).
   - **Universal Code Equivalence (Zero Second-Class Code)**: Every line of code across the entire codebase must be held to the exact same uncompromising, high quality standards during any task.
   - **Stage-Gated Senior Code Review**: Perform a thorough senior-level code review upon completion of each phase before advancing.
4. **Continuous Upstream Synchronization & Atomic Commits**:
   - **Zero Local Change Accumulation**: Never allow uncommitted or unpushed local changes to accumulate across development phases.
   - **Atomic Commit & Push on Gate Sign-Off**: Immediately commit and push upstream upon passing verification tiers.

---

## Phases

### Phase 1: Requirements Discovery & Dead Code Inventory
- [x] Scan and inventory all Windows VM provisioning code, bash scripts, and derivations across the repository.
- [x] Detail all dead code blocks in `middle-earth/hosts/gandalf/virtualization/images/windows.nix` (`buildApp`, `mkWin11Domain`, `domainXmls`, `provisionApp`, `backupApp`, `hostnameForVersion`, `versionLookupScript`).
- [x] Detail deprecated backward-compatibility app shims in `apps/default.nix` (`build-windows-image`, `provision-windows-vm`, `backup-windows-vm`).
- [x] Document complete inventory and modern requirements in `findings.md`.
- **Status:** complete

### Phase 2: Target Flake Module Architecture & Design Specification
- [x] Define the modernized, lean module interface for `windows.nix` exporting pure OEMDRV assets (`autounattendXml`, `mkSysprepXml`, `provisionPs1`, `errorHandlerCmd`) and `uupEnv` / `buildIsoApp`.
- [x] Define integration with `onehost.nix`: ensure `onehost.nix` cleanly consumes OEMDRV assets, compiles `onehost.json`, and exposes all `onehost-*` apps including `onehost-build-iso`.
- [x] Define clean flake app manifest in `apps/default.nix` eliminating all backward-compatibility shims.
- [x] Review plan and architectural specification.
- **Status:** complete

### Phase 3: Refactor Windows Module (`windows.nix` & `windows-versions.nix`)
- [x] Prune ~730 lines of dead code from `middle-earth/hosts/gandalf/virtualization/images/windows.nix`:
  - Remove `buildApp` (`build-windows-image` bash script).
  - Remove `mkWin11Domain` and `domainXmls`.
  - Remove `provisionInstance`, `provisionAllScript`, `provisionTargetsScript`, `provisionApp` (`provision-windows-vm` bash script).
  - Remove `backupAllScript`, `backupApp` (`backup-windows-vm` bash script).
  - Remove `instanceForVersion`, `hostnameForVersion`, and `versionLookupScript`.
- [x] Refactor pure Windows unattended assets:
  - `mkAutounattendXml` / `autounattendXml`
  - `mkSysprepXml`
  - `provisionPs1`
  - `errorHandlerCmd`
- [x] Implement clean `buildIsoApp` (`onehost-build-iso`) utilizing `uupEnv` to download and generate `win11-${version}.iso` directly into `/data/depot/virtualization/libvirt/iso/`.
- [x] Update `createFlakeModule` exports in `windows.nix`.
- [x] Verify `nix eval .#middle-earth.hosts.gandalf.virtualization.images.windows` succeeds.
- **Status:** complete

### Phase 4: Flake App Eradication & Modernization in `apps/default.nix` & `onehost.nix`
- [x] Wire `buildIsoApp` into `onehost.nix` apps set as `onehost-build-iso`.
- [x] Refactor `apps/default.nix`:
  - Remove deprecated `build-windows-image`.
  - Remove deprecated `provision-windows-vm`.
  - Remove deprecated `backup-windows-vm`.
  - Expose modern apps: `onehost`, `onehost-plan`, `onehost-apply`, `onehost-destroy`, `onehost-backup`, `onehost-restore`, `onehost-status`, `onehost-build-image`, `onehost-build-iso`.
- [x] Verify `nix flake show` displays the clean, unified application list.
- **Status:** complete

### Phase 5: Verification, Nix Flake Check & Live Hypervisor Validation
- [x] Verify hermetic Nix build: `nix build .#packages.x86_64-linux.onehost --no-link`.
- [x] Verify manifest derivation: `nix build .#packages.x86_64-linux.onehost-manifest --no-link`.
- [x] Run full Rust test suite: `nix shell nixpkgs#cargo nixpkgs#gcc nixpkgs#qemu-utils --command cargo test --all-targets` (140/140 tests pass).
- [x] Run Clippy audit: `nix shell nixpkgs#cargo nixpkgs#clippy --command cargo clippy --all-targets -- -D warnings` (0 warnings).
- [x] Run Nix flake check: `nix flake check`.
- [x] Run live verification on Gandalf: `nix run .#onehost-plan` and `nix run .#onehost-status`.
- **Status:** complete

### Phase 6: Upstream Synchronization & Delivery
- [x] Perform static invariant audit across the repository.
- [x] Verify pristine git working tree.
- [x] Commit with conventional commit message (`refactor(virtualization): prune dead legacy scripts and unify on onehost`) and push to `origin/main`.
- [x] Present comprehensive delivery report to user.
- **Status:** complete

---

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Zero Backwards Compatibility | Per user instruction and Pillar I Rule 3, no deprecated shims, legacy bash scripts, or transitional aliases are retained. |
| Prune 730+ lines of Dead Code in `windows.nix` | `buildApp`, `provisionApp`, `backupApp`, `mkWin11Domain`, `domainXmls` are completely redundant with `onehost`. Removing them establishes a single source of truth. |
| Generalized Golden Master Answer Files | Eliminate VM-specific hostname injection into `autounattend.xml` and `sysprep.xml`. Golden masters must be generalized; individual VM hostnames are applied post-provisioning or via cloud-init/sysprep generalization. |
| Dedicated `onehost-build-iso` App | Decouple ISO acquisition (via UUP dump) from VM lifecycle management, providing a clean command for fetching Windows ISOs directly to `/data/depot/virtualization/libvirt/iso/win11-${version}.iso`. |
| Clean App Namespace | Expose only `onehost` and `onehost-*` in `apps/default.nix`, deprecating `build-windows-image`, `provision-windows-vm`, and `backup-windows-vm`. |

---

## Errors Encountered
| Error | Resolution |
|-------|------------|
