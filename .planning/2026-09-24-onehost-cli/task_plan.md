# Task Plan: Onehost CLI Planning & Implementation

## Goal
Plan and execute the development of the `onehost` Rust CLI tool (at packages/onehost) for declarative Windows libvirt KVM lifecycle management, refactoring existing bash apps into a test-driven, declarative tool configured via Nix flake derivations with XDG compliance, zero implicit defaults, and OpenTofu-style reconciliation—using Domain Template XML with explicit `<disk onehost:role='os-disk'>` markers and content-addressed OEMDRV flavor derivations.

## Next Step
Await user signal to begin Phase 15: Stage 10 - Nix Flake Derivation, Module, and App Integration.

## Current Phase
Phase 14: Stage 9 - Image Builder Pipeline & CLI Wiring (Complete)

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

## Phases

### Phase 1: Requirements & Discovery
- [x] Explore network topology (sauron IPv6 /48 static routing, firewall rules)
- [x] Explore OpenTofu incus containers & gameserver configurations (terranix / opentofu patterns)
- [x] Explore persistent storage and cloud backup mechanisms
- [x] Explore existing Windows KVM scripts: build-windows-image, provision-windows-vm, backup-windows-vm
- [x] Inspect packages/onehost current state
- [x] Document findings in findings.md
- **Status:** complete

### Phase 2: Architectural Synthesis & Interview Preparation
- [x] Identify behaviors to refactor from Bash/Nix into Rust `onehost`
- [x] Formulate design alternatives for declarative configuration, CLI UX, KVM/libvirt interaction, XDG compliance, and TDD strategy
- [x] Prepare targeted interview questions to align on implementation details
- **Status:** complete

### Phase 3: User Interview & Requirements Alignment
- [x] Conduct interactive interview with the user
- [x] Clarify decisions on KVM lifecycle, state management, storage/backup, and Nix integration
- **Status:** complete

### Phase 4: Architecture Specification & Roadmap Definition
- [x] Write comprehensive architecture specification and step-by-step development roadmap
- [x] Refactor architecture: Domain Template XML with explicit `onehost:role='os-disk'`
- [x] Refactor image tags: `<os>/<version>/<flavor>` backed by content-addressed OEMDRV derivations (eliminating manual revisions)
- [x] Define Rust module layout, manifest schema, and testing strategy (TDD)
- [x] Document updated schemas, diagrams, and trait definitions in findings.md
- **Status:** complete

### Phase 5: Architecture & Plan Review with User
- [x] Present Domain Template with `onehost:role='os-disk'` and content-addressed OEMDRV flavor hashing architecture
- [x] Review implementation details and address any adjustments or questions (lifecycle guardrails, storage pool relocation, backup engine, invariants)
- [x] Gain final sign-off before commencing Rust code changes
- **Status:** complete

### Phase 6: Stage 1 - Scaffolding, Dependencies, and XDG Core (TDD)
- [x] Update `Cargo.toml` with required dependencies (clap, serde, serde_json, thiserror, quick-xml, tempfile, tracing, tracing-subscriber)
- [x] Write unit tests for XDG Base Directory resolution (`tests/xdg_directory_tests.rs`)
- [x] Implement `src/xdg.rs` resolving `$XDG_CONFIG_HOME`, `$XDG_CACHE_HOME`, `$XDG_STATE_HOME`, `$XDG_RUNTIME_DIR` with strict error handling
- [x] Verify `cargo test` passes cleanly with zero warnings
- [x] Verify `nix build .#packages.x86_64-linux.onehost` passes
- [x] Conduct senior-level code review of Stage 1
- **Status:** complete

### Phase 7: Stage 2 - Image Tag & Content-Addressed Flavor Resolver (TDD)
- [x] Write unit tests in `tests/image_tag_tests.rs`:
  - Tag parsing (`<os>/<version>/<flavor>`)
  - Mapping flavor to OEMDRV derivation path and hash
  - Rejection of unknown flavors or malformed tags (and legacy 4-part tags)
  - Derivation of golden master depot paths: `${os}-${version}-${flavor}-${hash}.qcow2`
  - Derivation of local base cache and instance overlay paths
- [x] Implement `src/image/tag.rs` (`ImageTagSpecification`, `FlavorDerivationMetadata`, `ContentAddressedImageResolver`)
- [x] Verify `cargo test` passes cleanly (13 tests total)
- [x] Verify `nix build .#packages.x86_64-linux.onehost` passes
- [x] Conduct senior-level code review of Stage 2
- **Status:** complete

### Phase 8: Stage 3 - Declarative Manifest Model & Strict Validation (TDD)
- [x] Write unit tests in `tests/config_tests.rs`:
  - Valid manifest deserialization (`flavors` map with OEMDRV paths/hashes, `instances` map with pool and `lifecycle: { on_image_change, prevent_destroy }`, storage directories)
  - Missing field rejection (zero implicit defaults)
  - Path format validation and existence checks
- [x] Implement `src/config/model.rs`, `src/config/validation.rs`, and `src/config/loader.rs`
- [x] Verify `cargo test` passes cleanly (23 tests total)
- [x] Verify `nix build .#packages.x86_64-linux.onehost` passes
- [x] Conduct senior-level code review of Stage 3
- **Status:** complete

### Phase 9: Stage 4 - Domain Template Engine & Disk Injection (TDD)
- [x] Write unit tests in `tests/template_injection_tests.rs`:
  - Locating `<disk onehost:role='os-disk'>` under declared `xmlns:onehost`
  - Preserving existing `<target dev='...' bus='...'/>` from the template
  - Stripping the `onehost:role` attribute and root `xmlns:onehost`, and injecting `<driver name='qemu' type='qcow2'/>` and `<source file='...'/>` pointing to instance CoW overlay
  - Leaving unmanaged secondary disks intact
  - Injecting instance `<name>`, `<uuid>`, and `<nvram>`
  - Failing validation if 0 or multiple disks have `onehost:role='os-disk'`
  - Failing validation if `<name>` or `<uuid>` elements are missing
- [x] Implement `src/domain/template.rs`
- [x] Verify `cargo test` passes cleanly (30 tests total)
- [x] Verify `nix build .#packages.x86_64-linux.onehost` passes
- [x] Conduct senior-level code review of Stage 4
- **Status:** complete

### Phase 10: Stage 5 - Domain Live Diff Engine & Normalization (TDD)
- [x] Write unit tests in `tests/domain_diff_tests.rs`:
  - Normalizing Libvirt XML (filtering volatile hypervisor runtime attributes: dynamic `id`, `<alias>`, auto-allocated PCI bus/slots, default `<address>`, and auto-generated MACs)
  - Detecting semantic drift between synthesized concrete XML and live `virsh dumpxml`
- [x] Implement `src/domain/diff.rs`
- [x] Verify `cargo test` passes cleanly (41 tests total)
- [x] Verify `nix build .#packages.x86_64-linux.onehost` passes
- [x] Conduct senior-level code review of Stage 5
- **Status:** complete

### Phase 10b: Codebase Review & Functional Modernization (Stages 1–5)
- [x] Read all planning files in full (`task_plan.md`, `findings.md`, `progress.md`)
- [x] Review and refactor `src/xdg.rs`: functional monadic combinators (`map`, `and_then`), iterator pipelines, immutable error handling
- [x] Review and refactor `src/image/tag.rs`: ensure born-valid types, monadic resolution
- [x] Review and refactor `src/config/`: `OnehostManifest::new` enforces validation upon creation so manifests are born valid; functional iterator validation in `validation.rs`
- [x] Review and refactor `src/domain/template.rs`: refactor from imperative mutable boolean flags to pure functional AST tree transformation
- [x] Review and refactor `src/domain/diff.rs`: refactor from `&mut DomainXmlElement` and `&mut Vec<DomainXmlDifference>` to pure functional transformations `(DomainXmlElement, DomainXmlElement) -> (DomainXmlElement, DomainXmlElement)` and `calculate_element_differences(...) -> Vec<DomainXmlDifference>` using iterators and monadic combinators
- [x] Verify `cargo test` passes cleanly across all 41+ tests with zero warnings
- [x] Verify `nix build .#packages.x86_64-linux.onehost` passes
- [x] Conduct senior-level code review of functional refactoring
- [x] Pause and request user approval before proceeding to Phase 11
- **Status:** complete

### Phase 10c: Comprehensive Senior-Level Deep Review & Refinement of Phase 10b Paradigm Transition
- [x] **Track 1: Move Semantics & Zero-Copy AST Purity Audit**
  - Refactored `src/domain/template.rs`: eliminated borrow-and-clone patterns (`&DomainXmlElement` in helpers); consumed `template_root` and child nodes by value using `transform_children`; eliminated manual child and attribute cloning.
  - Refactored `src/domain/diff.rs`: updated `normalize_domain_trees(synthesized, live)` and `sort_element_children_deterministically(element)` to consume inputs by move, eliminating redundant clones when normalizing freshly parsed ASTs.
  - Refactored `src/domain/element.rs`: in `parse_element_body`, consumed `parsed_items` via `.into_iter()` moving `BodyItem::Child` directly without `.clone()`.
  - Made `DomainXmlElement` fields public (`pub tag_name`, `pub attributes`, `pub children`, `pub text_content`) to treat AST nodes as pure data, enabling direct pattern matching, destructuring, and struct update syntax while eliminating boilerplate intermediate parts structs.
  - Fixed mixed-content and text node handling in `element.rs::to_xml_string()`: verified that child nodes and text nodes are never dropped.
- [x] **Track 2: Monadic Purity & Clippy / Compiler Linter Rigor Audit**
  - Fixed `clippy::collapsible_if` in `src/config/model.rs:222-232`: replaced nested `if let` blocks with monadic `self.pool().map(str::trim).filter(|pool| !pool.is_empty()).or_else(...)`.
  - Fixed `clippy::type_complexity` in `src/xdg.rs:30`: extracted `pub type EnvironmentLookup = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;`.
  - In `src/xdg.rs:151-160`: replaced imperative `if let ... else` with monadic `map().unwrap_or_else()`.
  - In `src/config/validation.rs:119-126`: eliminated duplicated pool validation logic by calling `instance_config.effective_pool(storage)`.
  - Verified `cargo clippy --all-targets -- -D warnings` compiles cleanly with zero warnings.
- [x] **Track 3: Descriptive Naming & Guideline Compliance Audit (Rule 3, 8, 9)**
  - Audited and eradicated all single-letter closure variables across the codebase:
    - `src/config/validation.rs`: `|p|` -> `|pool|`, `|c|` -> `|character|`.
    - `src/domain/template.rs`: `|c|` -> `|child_element|`, `|(k, v)|` -> `|(attribute_key, attribute_val)|`.
    - `src/domain/diff.rs`: `|c|` -> `|child_element|`, `|t|` -> `|target_element|`, `|s|` -> `|source_element|`, `|a|` -> `|address_element|`.
    - `src/domain/element.rs`: `|c|` -> `|child|`, `|b|` -> `|byte|`, `|e|` -> `|error|`, `|s|` -> `|string_slice|`.
    - `tests/domain_diff_tests.rs`: `|d|` -> `|difference|`.
  - Verified Rule 8 (born-valid types upon construction, zero unvalidated state) across all models.
  - Verified Rule 9 (`fn(self, ...) -> Self` builders by move, explicit call-site cloning).
- [x] **Track 4: Round-Trip AST Invariance & Edge Case Test Suite Expansion**
  - Added dedicated AST test suite (`tests/domain_element_tests.rs`):
    - Tested AST round-trip invariance: parsed complex Domain XML -> `to_xml_string()` -> re-parsed -> asserted structural equality (`original == reparsed`).
    - Tested self-closing empty elements, text-only elements, and nested children.
    - Tested zero-copy destructuring of `DomainXmlElement`.
    - Tested edge cases: malformed XML, unclosed tags, mismatched closing tags, trailing text, empty string rejection.
- [x] **Track 5: Verification & Hermetic Nix Flake Build**
  - Verified full test suite passes cleanly with zero warnings (`cargo test`, 47/47 tests passing).
  - Verified zero clippy warnings (`cargo clippy --all-targets -- -D warnings`).
  - Verified hermetic Nix flake package build (`nix build .#packages.x86_64-linux.onehost --no-link`).
  - Conducted final senior-level code review.
- [x] **Track 6: Rerun & Deep Oversights Audit - Preference of Public Attributes over Getters (Rule 10)**
  - Promoted fields to `pub` and eradicated trivial boilerplate getters across all data models and ASTs:
    - `ImageTagSpecification`: `pub operating_system`, `pub build_version`, `pub flavor_name`.
    - `FlavorDerivationMetadata`: `pub oemdrv_nix_store_path`, `pub content_hash`.
    - `OnehostManifest`: `pub schema`, `pub version`, `pub storage`, `pub flavors`, `pub instances`.
    - `StorageDirectoriesConfiguration`: `pub depot_store_dir`, `pub depot_iso_dir`, `pub depot_backup_dir`, `pub nvram_dir`, `pub nvram_template`, `pub ovmf_code`, `pub default_pool`.
    - `FlavorConfiguration`: `pub oemdrv_path`, `pub hash`.
    - `InstanceConfiguration`: `pub uuid`, `pub template_xml`, `pub image`, `pub pool`, `pub autostart`, `pub lifecycle`.
    - `InstanceLifecycleConfiguration`: `pub on_image_change`, `pub prevent_destroy`.
    - `DomainXmlElement`: removed `tag_name(&self)`, `attributes(&self)`, `children(&self)`, `text_content(&self)` getters, utilizing direct public field access everywhere.
  - Refactored all call sites across `src/` and `tests/` (`validation.rs`, `diff.rs`, `template.rs`, `config_tests.rs`, `image_tag_tests.rs`, `domain_element_tests.rs`) to directly access public fields.
  - Eradicated single-letter closure variable pattern `|(k, v)|` -> `|(env_key, env_value)|` across 6 test cases in `tests/xdg_directory_tests.rs`.
  - Removed unused import `Path` in `src/config/model.rs`.
  - Re-verified full test suite (47/47 passing), zero clippy warnings (`-D warnings`), and hermetic Nix flake package build.
- **Status:** complete

### Phase 11: Stage 6 - Hypervisor and Storage Abstractions (TDD)
- [x] Define `trait Hypervisor` and `trait StorageManager` in `src/hypervisor/traits.rs` and `src/storage/traits.rs`:
  - `StorageManager` includes `resolve_pool_path(pool: &str)`, `refresh_pool(pool: &str)`, and cross-device safe move (`std::fs::rename` with `EXDEV` streaming copy fallback)
- [x] Implement `MockHypervisor` and `MockStorageManager` for hermetic testing
- [x] Implement production `VirshHypervisor` and `QemuImgStorage`
- [x] Write tests verifying mock behaviors, pool resolution, and cross-device move fallback (`tests/hypervisor_tests.rs`, `tests/storage_tests.rs`)
- [x] Verify `cargo test` passes cleanly (67 tests total)
- [x] Verify `cargo clippy --all-targets -- -D warnings` passes with 0 warnings
- [x] Verify `nix build .#packages.x86_64-linux.onehost` passes
- [x] Conduct senior-level code review of Stage 6
- **Status:** complete

### Phase 12: Stage 7 - Stateless Planner, Applier, and Destroyer (TDD)
- [x] Write unit tests in `tests/planner_tests.rs`:
  - Diff calculation: Create, Update (XML drift), Recreate (base image hash changed - forces replacement with lifecycle policy annotations), Relocate (pool changed with unchanged base image), Delete, NoOp
  - Active device inspection: detecting dangling `.snap` files from interrupted backups and flagging auto-recommit
- [x] Write unit tests in `tests/lifecycle_tests.rs`:
  - Applier sequence: resolve pool path from Libvirt -> lifecycle policy check (`prevent_destroy` aborts; `on_image_change == protect` aborts unless `--allow-recreate` / TTY prompt; `on_image_change == recreate` proceeds) -> base image sync -> CoW overlay management (preserve, recreate, or safe relocation across pools via `qemu-img rebase -u`) -> NVRAM init -> `virsh define <concrete_xml>` -> `virsh pool-refresh`
  - Relocation sequence: verify VM shut off -> ensure base cached in target pool -> move overlay -> rebase backing pointer -> define XML -> refresh both pools
  - Destroyer sequence: lifecycle policy check (`prevent_destroy` aborts unless `--allow-destroy-protected`) -> domain shutdown -> undefine -> overlay disk removal -> pool refresh
- [x] Implement `src/lifecycle/planner.rs`, `applier.rs`, and `destroyer.rs`
- [x] Verify `cargo test` passes cleanly (89 tests total)
- [x] Verify `cargo clippy --all-targets -- -D warnings` passes with 0 warnings
- [x] Verify `nix build .#packages.x86_64-linux.onehost` passes
- [x] Conduct senior-level code review of Stage 7
- **Status:** complete

### Phase 12b: Lifecycle Pipeline & Real Toolchain Integration Suite
- [x] Tier 1: Write cross-module pipeline integration tests in `tests/pipeline_integration_tests.rs`:
  - Workspace with real `onehost.json` manifest and domain template XML with `xmlns:onehost`
  - Integration: `ManifestLoader` -> `ContentAddressedImageResolver` -> `DomainTemplateEngine` -> `DomainLifecyclePlanner` -> `DomainLifecycleApplier`
  - Fresh provisioning end-to-end
  - Template drift detection and non-destructive reconciliation
  - Safe storage pool relocation across pools
  - Policy enforcement (`on_image_change: protect` aborts without `--allow-recreate`, `prevent_destroy` aborts)
- [x] Tier 2: Write real `qemu-img` toolchain integration tests in `tests/storage_toolchain_tests.rs`:
  - Add `pkgs.qemu-utils` to `nativeCheckInputs` in `packages/onehost/default.nix` for hermetic Nix execution
  - Real QCOW2 master creation and live inspection via `qemu-img info --output=json`
  - Real CoW overlay creation and backing pointer verification
  - Real metadata-only rebase (`qemu-img rebase -u`) and backing pointer update
  - Real image integrity checks (`qemu-img check`)
- [x] Verify `cargo test` passes cleanly across all tests (99 tests total)
- [x] Verify `cargo clippy --all-targets -- -D warnings` reports 0 warnings
- [x] Verify hermetic `nix build .#packages.x86_64-linux.onehost` passes
- [x] Conduct senior-level code review of Phase 12b
- **Status:** complete

### Phase 12c: Major Codebase Audit & Functional Standards Refactoring
*Context Compaction Invariant: Every sub-phase (12c.1 through 12c.5) MUST begin with the strict Fresh Read Protocol (reading the entire `## Mandatory Design Guidelines & Engineering Standards` section in full without skipping, accompanied by the target phase tasks from `task_plan.md`, latest state from `progress.md`, and relevant contracts from `findings.md`) to maintain context integrity and guarantee 100% compliance with the Triad of Foundations.*

#### Phase 12c.1: Core Foundation & Configuration Audit (`src/xdg.rs`, `src/image/tag.rs`, `src/config/`)
- [x] Fresh read of planning files (`task_plan.md`, `findings.md`, `progress.md`)
- [x] Audit & refactor `src/xdg.rs`:
  - Enforce total functions (zero `.unwrap()` / `.expect()`)
  - Enforce born-valid records with `pub` fields (no getters/setters)
  - Verify parameter slice borrowing (`&Path`, `&str`) vs owned return types (`PathBuf`)
  - Check descriptive naming (strictly zero single-letter variables, no type suffixes)
  - Verify pure monadic error propagation with `?` and expressions over statements
- [x] Audit & refactor `src/image/tag.rs`:
  - Enforce born-valid records with `pub` fields (no getters/setters)
  - Verify strict 3-segment tag parsing via pure iterator pipelines
  - Verify content-addressed flavor resolver logic and zero implicit defaults
  - Check descriptive naming across all closures and bindings
- [x] Audit & refactor `src/config/` (`model.rs`, `validation.rs`, `loader.rs`):
  - Audit all configuration models: ensure pure algebraic records with `pub` fields; eliminate any boilerplate OOP getters
  - Verify born-valid validation contract: `OnehostManifest::from_str` or `validate` executes complete semantic checks
  - Audit `validation.rs`: ensure pure functional iterator pipelines (`try_for_each`), descriptive naming, no loops, expressions over statements
  - Audit `loader.rs`: pure file read & parse without side-effects or logging to stdout
- [x] Run test suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, hermetic Nix flake package build
- [x] Conduct Stage-Gated Senior Code Review for Phase 12c.1
- **Status:** complete

#### Phase 12c.2: Domain XML AST, Synthesis & Normalization Audit (`src/domain/element.rs`, `src/domain/template.rs`, `src/domain/diff.rs`)
- [x] Fresh read of planning files (`task_plan.md`, `findings.md`, `progress.md`)
- [x] Audit & refactor `src/domain/element.rs`:
  - Verify immutable AST representation and pure builder pattern (`fn with_*(mut self, ...) -> Self`)
  - Ensure public field accessibility on AST records where appropriate
  - Enforce total functions: zero `.unwrap()` / `.expect()`
  - Verify parameter slices (`&str`, `&[DomainXmlElement]`) vs owned transformations
  - Audit naming conventions (strictly zero single-letter variables)
- [x] Audit & refactor `src/domain/template.rs`:
  - Verify pure Functional Core boundary: transformation takes input string/AST and returns synthesized AST/XML with zero I/O
  - Enforce strict validation of `xmlns:onehost` and `<disk onehost:role='os-disk'>`
  - Audit iterator pipelines: `.map()` for pure projections, monadic `?` error chaining, zero imperative loops
  - Ensure zero `.unwrap()` / `.expect()` in error paths
- [x] Audit & refactor `src/domain/diff.rs`:
  - Verify normalization engine: pure functional AST transformations removing runtime attributes (`id`, `<alias>`, dynamic `<seclabel>`, MAC/PCI addresses)
  - Verify differ: pure functional tree comparison generating typed difference AST
  - Eliminate any remaining imperative mutations, statements, or non-descriptive bindings
- [x] Run test suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, hermetic Nix flake package build
- [x] Conduct Stage-Gated Senior Code Review for Phase 12c.2
- **Status:** complete

#### Phase 12c.3: Trait Abstractions & Infrastructure Adapters Audit (`src/hypervisor/`, `src/storage/`)
- [x] Fresh read of planning files (`task_plan.md`, `findings.md`, `progress.md`)
- [x] Audit & refactor `src/hypervisor/` (`mod.rs`, `virsh.rs`, `mock.rs`):
  - Verify Trait Boundary Contract: `Hypervisor` trait defines clean hardware interface
  - Audit `virsh.rs`: pure command building, streaming error capture, `try_for_each` for multi-step operations, parameter borrowing (`&str`, `&Path`)
  - Audit `mock.rs`: verify in-memory mock isolation without host leaks
  - Enforce zero `.unwrap()` / `.expect()` across production code
  - Enforce zero single-letter variables and zero type suffixes (`_str`, `_vec`)
- [x] Audit & refactor `src/storage/` (`mod.rs`, `qemu_img.rs`, `mock.rs`):
  - Verify Trait Boundary Contract: `StorageManager` encapsulates all QEMU image and filesystem mutations
  - Audit `qemu_img.rs`: verify `qemu-img` command executions, robust JSON info parsing, atomic promotion semantics, `EXDEV` cross-device fallback
  - Ensure `StorageManager::delete_image` encapsulates disk cleanup (zero raw `fs::remove_file` in callers)
  - Audit `mock.rs`: verify pure in-memory tracking of images, overlays, backing chains, and pool discovery
  - Enforce zero `.unwrap()` / `.expect()` across production code
- [x] Run test suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, hermetic Nix flake package build
- [x] Conduct Stage-Gated Senior Code Review for Phase 12c.3
- **Status:** complete

#### Phase 12c.4: Lifecycle Reconciliation & Integration Pipelines Audit (`src/lifecycle/`, `tests/pipeline_integration_tests.rs`, `tests/storage_toolchain_tests.rs`)
- [x] Fresh read of planning files via Fresh Read Protocol (full Standards section in whole together + target phase context bundle)
- [x] Audit & refactor `src/lifecycle/planner.rs`:
  - Verify Query Shell vs. Pure Core separation: query traits for live snapshots, delegate drift computation to pure Core differ, emit immutable `OnehostPlan`
  - Ensure pure iterator pipelines: `.map()` for pure projections, `.fold()` / `.try_for_each()` for accumulation
  - Enforce guardrail detection: accurately classify and tag destructive actions requiring `--allow-recreate` or `--allow-destroy-protected`
  - Zero single-letter closure variables, zero `.unwrap()` / `.expect()`
- [x] Audit & refactor `src/lifecycle/applier.rs`:
  - Verify Execution Shell linear orchestration: execute plan actions via traits
  - Audit iterator semantics: `.try_for_each()` for effectful execution, zero imperative `for` loops
  - Verify non-destructive guardrails: abort safely on unpermitted recreation or pool relocation
  - Verify RAII safety and error handling with typed `thiserror` variants
- [x] Audit & refactor `src/lifecycle/destroyer.rs`:
  - Verify lifecycle policy enforcement (`prevent_destroy` aborts unless `--allow-destroy-protected`)
  - Ensure pure pipeline orchestration of domain shutdown, undefine, and storage cleanup via `StorageManager`
- [x] Audit & refactor integration tests:
  - Verify `tests/pipeline_integration_tests.rs` and `tests/storage_toolchain_tests.rs` follow clean TDD style, no brittle host coupling, hermetic Nix environment execution
- [x] Run test suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, hermetic Nix flake package build
- [x] Conduct Stage-Gated Senior Code Review for Phase 12c.4
- **Status:** complete

#### Phase 12c.5: Whole-System Verification & Senior Gate Review
- [x] Fresh read of planning files via Fresh Read Protocol (full Standards section in whole together + target phase context bundle)
- [x] Full codebase static analysis:
  - Grep audit: verify 0 `for`, `while`, or `loop` statements across `packages/onehost/src/` (remediated 3 legacy reader loops in `element.rs` to pure tail recursion)
  - Grep audit: verify 0 `.unwrap()` and 0 `.expect()` across `packages/onehost/src/`
  - Grep audit: verify 0 single-letter closure/variable names across `packages/onehost/src/`
  - Grep audit: verify 0 `println!` or raw stdout writes in library code (`packages/onehost/src/` except intended CLI printer)
  - Grep audit: verify 0 OOP getter/setter boilerplate on pure data structs
- [x] Verify Cargo metadata, documentation, and warning-free compilation:
  - `cargo check`
  - `cargo test --all-targets` (all 99+ tests passing)
  - `cargo clippy --all-targets -- -D warnings` (0 warnings)
  - `nix build .#packages.x86_64-linux.onehost --no-link` (hermetic flake build passes)
- [x] Conduct comprehensive Senior Gate Review across the entire codebase
- [x] Prepare codebase for Phase 13 (Backup & Restore Engine)
- **Status:** complete

#### Phase 12c.6: Comprehensive Architectural Analysis & Structural Refactoring
*Context Compaction Invariant: Must begin with the strict Fresh Read Protocol (reading the entire `## Mandatory Design Guidelines & Engineering Standards` section in whole together + target phase context bundle) to guarantee 100% adherence to Universal Code Equivalence and the Triad of Foundations.*
- [x] **Track 1: Architectural Surface Mapping & Dependency Graph Audit**:
  - Mapped internal crate dependency graph across `config`, `domain`, `hypervisor`, `storage`, and `lifecycle`.
  - Verified strict unidirectional, acyclic module dependencies (Core $\to$ Traits $\to$ Shell) with zero inverted layers.
- [x] **Track 2: Deep Domain Modeling & Type-Level Invariants Audit**:
  - Identified primitive obsession: `InstanceConfiguration.uuid` as raw unvalidated `String` upon `new()`.
  - Identified AST ergonomics gap: verbose repeated boilerplate across `template.rs`, `diff.rs`, and traits for nested XML lookups.
- [x] **Track 3: Trait Boundary & Hardware Interface Cohesion Audit**:
  - Discovered leaky trait boundary in `src/lifecycle/applier.rs`: direct calls to `fs::create_dir_all` and `fs::copy` for NVRAM bypassing `StorageManager`.
  - Identified trait leakage: `trait StorageManager` embeds default `std::fs` operations.
- [x] **Track 4: Universal Code Equivalence & Mock Fidelity Audit**:
  - Discovered host filesystem leaks in `MockStorageManager`: calls to `source_depot_path.exists()`, `std::fs::copy()`, `std::fs::rename()`, and `std::fs::remove_file()` mutating the real host.
- [x] **Track 5: Error Architecture & Failure Domain Harmonization Audit**:
  - Identified stringly-typed `LifecycleError::ConfigurationError` usages in `planner.rs` and `destroyer.rs` bypassing strongly-typed error variants.
  - Identified silent error swallowing in `destroyer.rs`: `let _ = undefine_domain(...)` discarding fatal hypervisor failures.
- [x] **Track 6: Forward-Compatibility Pressure Test for Phase 13 (Backup & Restore)**:
  - Identified missing trait capability: `StorageManager` needs `restore_thin_backup` to complete symmetry with `convert_thin_backup` for disaster recovery.
- [x] **Track 7: Architectural Refactoring Execution & Verification**:
  - [x] **Track 7.1: Domain Modeling & Types Refactoring**:
    - [x] Implement born-valid `InstanceUuid` newtype in `src/config/model.rs` enforcing RFC-4122 syntax on construction (`new()`, `parse()`, `FromStr`, `Display`, `Serialize`, `Deserialize`, `Deref`, `TryFrom`).
    - [x] Update `InstanceConfiguration.uuid` to use `InstanceUuid` and streamline `validation.rs`.
    - [x] Add functional query combinators to `DomainXmlElement` in `src/domain/element.rs`:
      - `child_attribute(&self, child_tag: &str, attribute_key: &str) -> Option<&str>`
      - `child_text(&self, child_tag: &str) -> Option<&str>`
      - `path_text(&self, tag_path: &[&str]) -> Option<&str>`
      - `find_path(&self, tag_path: &[&str]) -> Option<&DomainXmlElement>`
      - `find_children<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a DomainXmlElement>`
      - `has_attribute_value(&self, key: &str, value: &str) -> bool`
      - `has_matching_attribute(&self, predicate: impl Fn(&str, &str) -> bool) -> bool`
    - [x] Refactor call sites in `template.rs`, `diff.rs`, `hypervisor/mock.rs`, and `hypervisor/traits.rs` to use new combinators.
    - [x] Write unit tests for `InstanceUuid` and new `DomainXmlElement` combinators.
  - [x] **Track 7.2: Trait Boundary & Mock Hermeticity Refactoring**:
    - [x] Add `initialize_nvram(&self, template_path: &Path, destination_nvram_path: &Path) -> Result<(), StorageError>` to `trait StorageManager` in `src/storage/traits.rs`.
    - [x] Add `restore_thin_backup(&self, archive_path: &Path, destination_overlay_path: &Path, backing_file_path: Option<&Path>) -> Result<(), StorageError>` to `trait StorageManager`.
    - [x] Implement `initialize_nvram` and `restore_thin_backup` in `src/storage/qemu_img.rs` for `QemuImgStorage`.
    - [x] Implement `initialize_nvram` and `restore_thin_backup` in `src/storage/mock.rs` for `MockStorageManager` with pure in-memory tracking and `RecordedStorageAction` variants (`InitializeNvram`, `RestoreThinBackup`).
    - [x] Eradicate ALL `std::fs` operations (`exists()`, `copy()`, `rename()`, `remove_file()`, `set_permissions()`) from `MockStorageManager`, achieving 100% hermetic in-memory mock isolation.
    - [x] Refactor `DomainLifecycleApplier` in `src/lifecycle/applier.rs` to delegate NVRAM creation to `self.storage.initialize_nvram(...)`.
    - [x] Write unit tests verifying hermetic mock behaviors, `initialize_nvram`, and `restore_thin_backup`.
  - [x] **Track 7.3: Error Hierarchy Harmonization & Diagnostics**:
    - [x] Add `InstanceNotDeclared { instance_name: String }` and `ManifestValidationError(#[from] ManifestValidationError)` to `LifecycleError` in `src/lifecycle/mod.rs`.
    - [x] Refactor `planner.rs` to propagate `FlavorResolutionError::UnknownFlavor` directly.
    - [x] Refactor `destroyer.rs` to return `InstanceNotDeclared` and propagate `ManifestValidationError`.
    - [x] Refactor `destroyer.rs` to explicitly match `undefine_domain` results: treat `DomainNotFound` as idempotent no-op while bubbling up true infrastructure errors.
    - [x] Update affected unit tests to verify typed error assertions.
  - [x] **Track 7.4: Verification, 4-Tier Test Suite & Senior Gate Review**:
    - [x] Verify `cargo check` passes with 0 warnings.
    - [x] Verify `cargo test --all-targets` passes across all 105 tests.
    - [x] Verify `cargo clippy --all-targets -- -D warnings` passes with 0 warnings.
    - [x] Verify hermetic Nix flake package build (`nix build .#packages.x86_64-linux.onehost --no-link`).
    - [x] Conduct Stage-Gated Senior Code Review for Phase 12c.6.
- **Status:** complete

### Phase 13: Stage 8 - Online/Offline Thin Backup and Restore Engine (TDD)
- [x] Write unit tests in `tests/backup_tests.rs`:
  - Multi-disk atomic snapshotting: `--diskspec` for all attached disks simultaneously
  - Opportunistic VSS quiescing via `qemu-ga` with graceful fallback to crash-consistent snapshot
  - RAII cleanup guard: verifies `blockcommit --active --pivot` executes on conversion failure, panic, or cancellation
  - Thin extraction and zlib compression via `qemu-img convert -c -B <base> -F qcow2`
  - Structured `manifest.json` generation and validation
  - Restore engine: verifies single-command disaster recovery (`onehost restore <backup-dir>`) re-attaches base image, restores NVRAM, defines domain, and refreshes pool
- [x] Implement `src/backup/engine.rs` and `src/backup/restore.rs`
- [x] Verify `cargo test` passes cleanly (116 tests total)
- [x] Verify `cargo clippy --all-targets -- -D warnings` passes with 0 warnings
- [x] Verify `nix build .#packages.x86_64-linux.onehost --no-link` passes
- [x] Conduct senior-level code review of Stage 8
- **Status:** complete

### Phase 13b: Full Disaster Recovery & Backup Integration Suite
- [x] Write full disaster recovery integration tests in `tests/disaster_recovery_integration_tests.rs`:
  - Full round-trip loop: Provision via `apply` -> Backup (online/offline) -> Destroy (`--delete-disk`) -> Restore (`onehost restore`) -> Plan verification (asserts NoOp / zero drift)
  - Real `qemu-img convert -c` compression against base image
  - Automatic repopulation of missing local pool base cache from depot store
  - Multi-disk disaster recovery round-trip (`sda` OS disk + `sdb` data disk)
  - Guardrail validations (rejection of existing domain without `--allow-overwrite`, error on missing base image)
- [x] Verify `cargo test --all-targets` passes (124 tests total)
- [x] Verify `cargo clippy --all-targets -- -D warnings` passes with 0 warnings
- [x] Verify `nix build .#packages.x86_64-linux.onehost --no-link` passes
- [x] Conduct senior-level code review of Phase 13b
- **Status:** complete

### Phase 14: Stage 9 - Image Builder Pipeline & CLI Wiring
- [x] Write unit tests in `tests/builder_tests.rs`:
  - Ephemeral scratch isolation in `$XDG_CACHE_HOME/onehost/build/`
  - Atomic promotion sequence (`scratch` -> `${master}.tmp` -> `qemu-img check` -> `0444` `${master}.qcow2`)
- [x] Implement `src/image/builder.rs` orchestrating unattended build using the Nix store OEMDRV derivation and headless QEMU/swtpm
- [x] Implement `src/cli.rs` and `src/main.rs` dispatching commands (`plan`, `apply`, `destroy`, `backup`, `restore`, `image build`, `status`)
- [x] Write CLI integration tests in `tests/cli_tests.rs` (10 tests)
- [x] Verify `cargo test --all-targets` passes cleanly across 140 tests with zero warnings
- [x] Verify `cargo clippy --all-targets -- -D warnings` passes with 0 warnings
- [x] Verify `nix build .#packages.x86_64-linux.onehost --no-link` passes
- [x] Implement Antigravity lifecycle hooks (`.agent/hooks.json` and `.agent/scripts/compaction_guard.py`)
- [x] Conduct senior-level code review of Phase 14
- **Status:** complete

### Phase 15: Stage 10 - Nix Flake Derivation, Module, and App Integration
- [ ] Update `packages/onehost/default.nix` and `Cargo.lock`
- [ ] Create Nix module / derivation generating `onehost.json`, domain template XMLs, and OEMDRV derivations from `virtualization.instances`
- [ ] Expose flake apps in `apps/default.nix` (`onehost-plan`, `onehost-apply`, `onehost-backup`, `onehost-destroy`)
- [ ] Update `systemd.services.libvirt-backup-vms` in `kvm.nix` to use `onehost backup`
- [ ] Verify build with `nix build .#packages.x86_64-linux.onehost`
- **Status:** pending

### Phase 16: Verification, Full Test Suite, and Delivery
- [ ] Run full test suite with `cargo test`
- [ ] Run Nix flake checks with `nix flake check`
- [ ] Review implementation against all user requirements
- **Status:** pending

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Domain Template XML + `onehost:role='os-disk'` | Flake defines hardware topology via template XML; explicit attribute eliminates ambiguity when multiple disks exist. |
| Content-Addressed Flavor Hash (No Manual Revisions) | The OEMDRV Nix store derivation hash uniquely identifies the flavor's provisioning state. Eliminates manual `v1`/`v2` strings; enables automatic cache invalidation and drift detection. |
| Tag Format `<os>/<version>/<flavor>` | Clean and intuitive. Flavor maps to the Nix OEMDRV derivation path and hash. |
| Lean Manifest Schema | `onehost.json` coordinates instances, pointing to template XMLs, flavor OEMDRVs, UUIDs, and storage directories. |
| Unified CLI Scope | Consolidate `image build`, `plan`, `apply`, `destroy`, `backup`, and `status` under `onehost`. |
| OpenTofu Command Style | Familiar declarative reconciliation (`plan`, `apply`, `destroy`, `backup`). |
| Trait-Based Abstractions | Decouple `virsh`, `qemu-img`, `swtpm` behind traits for mock-driven TDD without requiring root/libvirtd in unit tests. |
| Zero Implicit Defaults | Strict JSON validation rejects missing/ambiguous fields to preserve declarative purity. |
| Staged Thin Backups | Zero-downtime live snapshots staged to persistent depot for daily cloud backup via Restic. |
| Strict XDG Compliance | Use `$XDG_CONFIG_HOME`, `$XDG_CACHE_HOME`, `$XDG_STATE_HOME`, `$XDG_RUNTIME_DIR`; eliminate hardcoded `/tmp`. |
| Declarative Lifecycle Policy & CLI Guardrails | Instances declare `lifecycle` (`on_image_change: protect | recreate`, `prevent_destroy: bool`) in manifest; CLI gates base image replacement with `--allow-recreate` flag / TTY prompt to prevent accidental data loss. |
| Automated Safe Storage Pool Relocation | Changing an instance's pool in manifest triggers a safe, non-destructive migration (`mv` + fast metadata-only `qemu-img rebase -u` + `virsh define`) instead of accidental destruction or rebuild. |
| Libvirt XML Normalization | Filters volatile hypervisor runtime attributes (`id`, `<alias>`, auto-allocated PCI slots, default addresses, auto-generated MACs) to eliminate false-positive drift. |
| Cross-Device Relocation (`EXDEV` Fallback) | Safely moves disks across mount points (NVMe `/var/lib/...` to Btrfs `/data/...`) via streaming copy fallback when `std::fs::rename` returns `EXDEV`. |
| Atomic Golden Master Promotion | Golden masters staged to `${master}.tmp`, verified with `qemu-img check`, and atomically renamed to `0444` `.qcow2` to prevent half-baked images from polluting the store. |
| Robust Thin Backup with RAII Guard | Multi-disk atomic snapshotting, opportunistic VSS quiescing via `qemu-ga`, and RAII `blockcommit` cleanup guard to permanently eliminate runaway dangling `.snap` chains. |
| Structured Backup Manifest & 1-Command Restore | Staged backups include strongly-typed `manifest.json` enabling single-command disaster recovery (`onehost restore <backup-dir>`). |
| Multi-Tier Integration Testing (Hybrid Approach) | Validates cross-module pipeline integration (Manifest + Template + Planner + Applier) and real `qemu-img` CLI toolchain in Phase 12b before adding Backup/Restore, and validates complete disaster recovery round-trip in Phase 13b. |

## Errors Encountered
| Error | Resolution |
|-------|------------|
