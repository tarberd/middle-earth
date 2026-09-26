# Antigravity Workspace Rules: middle-earth

## Foundational Principle: Universal Code Equivalence & Absolute Standards
> **"NEVER assume code to be less critical or important to any task. All code across the entire codebase—whether pure domain logic, imperative execution shell, low-level parser routines, CLI dispatchers, error definitions, mock drivers, test harnesses, or integration suites—must be held to the exact same uncompromising, high quality standards during any task. There are zero second-class components, zero exemptions, and zero quality tiers."**

---

## Mandatory Post-Compaction & Phase Invariant: Fresh Read Protocol
1. **Compaction Detection**: Whenever context compaction occurs (indicated by the presence of a `<CONTEXT_SUMMARY>` block in the conversation prompt), or when starting/resuming any development phase, the agent MUST immediately perform a fresh read of the planning files via `view_file`:
   - `## Mandatory Design Guidelines & Engineering Standards` in `task_plan.md` in full (never skip or skim).
   - The target phase blueprint in `task_plan.md` (task checklist, scope, acceptance criteria).
   - Recent execution history in `progress.md`.
   - Relevant domain contracts, schemas, and invariants in `findings.md`.
2. **Anti-Assumption Guardrail**: Relying on summarized, truncated, or assumed memory of engineering standards or phase requirements after context compaction is strictly forbidden. The full files must be freshly ingested.

---

## Pillar I: Architectural Blueprint & System Contracts

### 1. Functional Core with an Imperative Shell
* **Functional Core (Pure Domain Logic)**:
  * AST parsing, validation, XML template transformation (`template.rs`), drift calculation (`diff.rs`), content-addressed image tag/hash resolution (`tag.rs`).
  * Pure transformations `fn(Input) -> Result<Output, Error>`. Zero filesystem I/O, zero network access, zero process execution. Tests for core logic require zero mocks.
* **Trait Boundary (Hardware & Infrastructure Abstraction)**:
  * Strict abstract interfaces (`Hypervisor`, `StorageManager`, `ProcessRunner`).
  * Production implementations wrap live CLI/system tools (`VirshHypervisor`, `QemuImgStorage`).
  * Test implementations provide hermetic in-memory mocks (`MockHypervisor`, `MockStorageManager`).
  * **Trait Purity Rule**: Traits must NOT embed default implementations with real host filesystem side-effects (e.g. raw `std::fs` calls). All I/O must be explicit in production implementors.
* **Imperative Shell (Orchestration, Planning & Reconciliation)**:
  * Query Shell (`DomainLifecyclePlanner`): Queries traits to inspect state, delegates drift comparison to pure core, emits immutable `OnehostPlan`.
  * Execution Shell (`DomainLifecycleApplier`, `DomainLifecycleDestroyer`, `BackupEngine`, `RestoreEngine`): Thin, linear orchestration sequentially executing actions via traits.
  * **Shell Trait Boundary Invariant**: The imperative shell must NEVER invoke `std::fs` or run external processes directly. All external mutations (including NVRAM directory creation and file copying) must be routed through trait methods (e.g. `StorageManager::initialize_nvram`).

### 2. Mock Hermeticity (100% In-Memory Isolation)
* Test mocks (`MockHypervisor`, `MockStorageManager`) must remain completely hermetic and isolated from the host environment.
* Calling `std::fs::copy`, `std::fs::rename`, `std::fs::remove_file`, `std::fs::create_dir_all`, `std::fs::set_permissions`, or checking host disk existence via `Path::exists()` inside mocks is strictly prohibited.
* Mocks must record actions in structured algebraic records (e.g. `RecordedStorageAction`, `RecordedHypervisorAction`) and track virtual state purely in-memory.

### 3. Declarative Purity & Zero Hidden State
* **Zero Implicit Defaults**: Every configuration value, hardware device, storage pool, path, and version must be explicitly declared or strictly derived. Missing or ambiguous fields fail fast during validation.
* **No Backwards Compatibility**: The tool represents a clean-slate standard. Legacy shims, deprecated tag structures, or historical migration paths are strictly prohibited.

### 4. Structured Errors & Zero Silent Swallowing
* Error hierarchies must be strongly typed with `thiserror`, categorized into Domain & Validation, Infrastructure & I/O, and Policy & Guardrails.
* **Zero Stringly-Typed Errors**: Do not emit dynamic strings in catch-all error variants (e.g. avoid `ConfigurationError { details: String }` when a typed variant like `InstanceNotDeclared` or `FlavorResolutionError::UnknownFlavor` can be used).
* **Zero Silent Error Swallowing**: Never discard fallible external operations with `let _ =`. When operations have expected idempotent conditions (such as undefining a domain that may already not exist), explicitly match and handle the expected variant (e.g., `HypervisorError::DomainNotFound => Ok(())`) while bubbling up true infrastructure errors.

---

## Pillar II: Functional Rust Craft & Idiomatic Implementation

### 1. Born-Valid Domain Types (Parse, Don't Validate Later)
* Wrap primitive types in strongly typed, born-valid newtypes (e.g. `InstanceUuid` enforcing RFC-4122 syntax upon construction).
* Structural invariants are enforced during construction (`new()`, `parse()`, `FromStr`, `TryFrom`). Once instantiated, a type is guaranteed valid; intermediate unvalidated states are prohibited.
* Implement ergonomic standard traits: `Display`, `Deref<Target=str>`, `AsRef<str>`, `PartialEq<&str>`, `Serialize`, and `Deserialize`.

### 2. Iterator Pipelines over Imperative Loops (Zero Loops Invariant)
* Imperative loops (`for`, `while`, `loop`) are strictly prohibited in `packages/onehost/src/`.
* Multi-item processing, filtering, searching, and collections must use declarative iterator pipelines (`.into_iter()`, `.iter()`, `.find()`, `.filter()`, `.map()`, `.try_for_each()`, `.fold()`, `.all()`, `.any()`, `std::iter::from_fn()`) or functional tail recursion.
* Use `.map()`, `.filter()`, and `.fold()` exclusively for pure data projections with zero side-effects. Use `.try_for_each()` or `.for_each()` exclusively when driving linear side-effects.

### 3. Control Flow: Expressions over Statements
* Branching must be value-producing expressions that compute results directly (`let value = if cond { a } else { b };`, `match` expressions, or monadic combinators like `.is_ok_and()`, `.is_some_and()`, `.and_then()`, `.or_else()`).
* Avoid mutable variables declared before conditional blocks and mutated inside statement branches.

### 4. Total Functions & Zero Panics
* Production code in `packages/onehost/src/` must be total; every fallible branch must be represented as a strongly typed `Result` or `Option`.
* Calling `.unwrap()` or `.expect()` anywhere in `packages/onehost/src/` is strictly prohibited (permitted only in unit test assertion bodies).

### 5. Semantic Naming (Zero Hungarian Notation, Zero Single-Letter Variables)
* Identifiers must communicate domain role, semantic intent, origin, or lifecycle state (`raw_template_xml`, `source_depot_path`, `overlay_disk_path`, `active_device`, `instance_configuration`).
* Technical type encoding in identifiers (`_str`, `_string`, `_vec`, `_map`, `_element`) is forbidden.
* Truncated single-letter variables (`|c|`, `|x|`, `|e|`, `p`, `img`) are strictly prohibited everywhere, including closure arguments.

---

## Pillar III: Rigorous Verification & Engineering Protocol

### 1. 4-Tier Verification Matrix
Before declaring any task or phase complete, all four tiers must pass with zero errors and zero warnings:
1. **Tier 1 & 2 (Check & Unit Tests)**:
   `nix shell nixpkgs#cargo nixpkgs#gcc nixpkgs#qemu-utils --command cargo test --all-targets`
2. **Tier 3 (Clippy Static Audit)**:
   `nix shell nixpkgs#cargo nixpkgs#clippy --command cargo clippy --all-targets -- -D warnings`
3. **Tier 4 (Hermetic Nix Build)**:
   `git add packages/onehost && nix build .#packages.x86_64-linux.onehost --no-link`
4. **Static Invariant Audit**:
   Verify 0 imperative loops (`grep -E '(^|\s)(for\s+\w+\s+in|while\s+|loop\s*\{)' src/`), 0 unwraps (`grep -E '\.(unwrap|expect)\(' src/`), and 0 single-letter closures (`grep -E '\|[a-zA-Z]\|' src/`).

### 2. Continuous Remote Synchronization & Atomic Commits
* **Zero Local Change Accumulation**: Never allow uncommitted or unpushed local changes to accumulate across development phases.
* **Atomic Commit & Push on Gate Sign-Off**: Upon completing each phase and passing all verification tiers, immediately commit all code, test, and planning files with conventional commit messages (`feat(...)`, `refactor(...)`, `docs(...)`) and push upstream (`git push origin <branch>`).
* Keep the git working tree clean between phases.
