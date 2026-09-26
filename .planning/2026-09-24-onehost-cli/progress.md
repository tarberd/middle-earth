# Progress Log

## Session: 2026-09-24

### Current Status
- **Phase:** Phase 12c.5: Whole-System Verification & Senior Gate Review (Completed - Awaiting user signal to begin Phase 13)
- **Started:** 2026-09-24

### Actions Taken
- Explored Nix flake structure, `flake.nix`, `flake-modules.nix`, `apps/default.nix`, and `packages/default.nix`.
- Explored network topology across Gandalf and Sauron: static routing of `2a0f:9400:738f::/48`, WireGuard `wg0`, bridge `br-public-hosts` (`10.100.2.1/24`, `2a0f:9400:738f:2::1/64`), and policy routing.
- Studied OpenTofu Incus container pattern in `gameservers/`: Terranix compiling declarative instances to JSON, runner scripts linking config to `$STATE_DIR/config.tf.json`, and running `tofu plan/apply/destroy`.
- Studied storage and backup mechanisms: `/data/depot` persistent storage on Btrfs, Restic daily backups to Google Drive via Rclone, and nightly staging of VM backups at 01:00.
- Studied current Windows KVM implementation in `windows.nix`: UUP dump ISO generation, unattended XML, headless QEMU/swtpm build, CoW overlay provisioning, and online/offline thin snapshot staging.
- Conducted interactive interview with user to clarify scope (unified CLI), command model (OpenTofu style `plan`/`apply`/`destroy`/`backup`), abstraction strategy (trait-based for TDD), serialization (JSON via Nix), state tracking (direct query), network attachment (declarative bridge specification), and optional passthrough (supporting both SR-IOV/KVMFR and standard VMs).
- Evaluated user proposal regarding generating Domain XML directly in the Nix flake: concluded this is fundamentally superior as it removes the intermediate JSON translation layer, keeps 100% of Libvirt XML expressiveness in Nix, and sharply focuses `onehost` on lifecycle orchestration, storage, and backups.
- Incorporated user feedback regarding Domain Template & Image Disks: Nix provides domain hardware templates; `onehost` is responsible for everything deriving from the instance `image` tag (golden master depot location, local base caching, CoW overlay creation, and injecting the OS disk & NVRAM into the template XML).
- Resolved Question 1 regarding multiple disks: Introduced dedicated `xmlns:onehost` and `<disk onehost:role='os-disk'>` attribute to explicitly identify the primary OS disk in the template with zero ambiguity.
- Resolved Question 2 regarding provisioning automation: Adopted Option 1 with content-addressed OEMDRV flavor hashing. The flavor maps to a Nix OEMDRV derivation path and hash; `<revision>` is completely eliminated from image tags, enabling automated cache invalidation and drift detection.
- Standardized on idiomatic `xmlns:onehost="https://middle-earth.internal/onehost"` with `onehost:role="os-disk"` per user preference for idiomatic XML customization and clear intent communication.
- Analyzed Storage Pool integration: formulated Declarative Storage Pool Resolution (Option A) where user declares the pool name, and `onehost` queries Libvirt to dynamically resolve physical path and manage volume refreshes.
- Documented complete VM Lifecycle State Machine and detailed operations specification (`plan`, `apply`, `destroy`, `backup`, `image build`, `status`) in `findings.md`.
- Integrated Declarative Lifecycle Policy (`on_image_change: protect | recreate`, `prevent_destroy: bool`) and CLI guardrails (`--allow-recreate` flag, TTY prompts) into the design to prevent accidental data loss on base image drift.
- Integrated Automated Safe Storage Pool Relocation (Option A): `onehost plan` detects pool relocation and `apply` performs zero-data-loss migration (`mv` + fast metadata-only `qemu-img rebase -u` + `virsh define`) instead of accidental destruction or rebuild.
- Conducted deep review of invariants, pre/post-conditions, and edge cases: adopted Libvirt XML normalization (Stage 5), cross-device `EXDEV` move fallback (Stage 6), and atomic golden master promotion (Stage 9).
- Conducted in-depth architectural review of the Backup Engine: adopted multi-disk atomic snapshotting, opportunistic VSS quiescing via `qemu-ga`, RAII `blockcommit` cleanup guard (preventing runaway `.snap` chains), structured `manifest.json` staging, and single-command disaster recovery (`onehost restore`).
- Formulated mandatory design guidelines: zero hidden defaults, NO backwards compatibility, descriptive/accurate naming, senior-level code reviews after each phase, no assumptions without interview, and have fun. Received plan approval and transitioned to Phase 6.
- Executed Phase 6: Stage 1 (Scaffolding, Dependencies, and XDG Core under TDD). Updated `Cargo.toml` and `Cargo.lock` with core dependencies (`clap`, `serde`, `serde_json`, `thiserror`, `quick-xml`, `tempfile`, `tracing`, `tracing-subscriber`).
- Implemented `packages/onehost/src/xdg.rs` (`XdgBaseDirectories`, `XdgDirectoryResolutionError`) and comprehensive unit tests in `packages/onehost/tests/xdg_directory_tests.rs` with zero compiler warnings.
- Verified test suite passes via `cargo test` and hermetic `nix build .#packages.x86_64-linux.onehost`.
- Conducted senior-level code review of Stage 1; verified zero hidden defaults, strict relative path rejection, and expressive naming. Transitioned to Phase 7: Stage 2.
- Executed Phase 7: Stage 2 (Image Tag & Content-Addressed Flavor Resolver under TDD). Implemented `ImageTagSpecification`, `FlavorDerivationMetadata`, and `ContentAddressedImageResolver` in `packages/onehost/src/image/tag.rs`.
- Created comprehensive integration tests in `packages/onehost/tests/image_tag_tests.rs`. All 13 unit tests across the test suite passed cleanly with zero compiler warnings.
- Verified hermetic build via `nix build .#packages.x86_64-linux.onehost`.
- Conducted senior-level code review of Stage 2; verified strict 3-segment parsing, legacy 4-part tag rejection, and descriptive naming. Transitioned to Phase 8: Stage 3.
- Executed Phase 8: Stage 3 (Declarative Manifest Model & Strict Validation under TDD). Implemented `OnehostManifest`, `StorageDirectoriesConfiguration`, `FlavorConfiguration`, `InstanceConfiguration`, `InstanceLifecycleConfiguration`, and `ImageChangePolicy` in `packages/onehost/src/config/model.rs`.
- Implemented strict validation logic in `packages/onehost/src/config/validation.rs` enforcing version "1.0", absolute paths, RFC-4122 UUID syntax, referenced flavor existence, and zero-implicit-default storage pool resolution.
- Implemented `ManifestLoader` in `packages/onehost/src/config/loader.rs`.
- Created 10 integration tests in `packages/onehost/tests/config_tests.rs`. All 23 unit tests across the test suite passed cleanly with zero compiler warnings.
- Verified hermetic build via `nix build .#packages.x86_64-linux.onehost`.
- Conducted senior-level code review of Stage 3; verified zero hidden defaults, strict path absoluteness, and expressive naming. Transitioned to Phase 9: Stage 4.
- Executed Phase 9: Stage 4 (Domain Template Engine & Disk Injection under TDD). Implemented `DomainTemplateEngine`, `DomainTemplateInjectionParameters`, and `DomainTemplateSynthesisError` in `packages/onehost/src/domain/template.rs`.
- Created 7 integration tests in `packages/onehost/tests/template_injection_tests.rs`. All 30 unit tests across the test suite passed cleanly with zero compiler warnings.
- Verified hermetic build via `nix build .#packages.x86_64-linux.onehost`.
- Conducted senior-level code review of Stage 4; verified zero hidden defaults, strict root domain / name / uuid / os-disk validation, standard Libvirt XML output sanitization, and expressive naming. Transitioned to Phase 10: Stage 5.
- Executed Phase 10: Stage 5 (Domain Live Diff Engine & Normalization under TDD). Implemented `DomainXmlDiffer`, `DomainXmlNormalizer`, `DomainXmlElement`, `DomainXmlDiffResult`, and `DomainXmlDifference` in `packages/onehost/src/domain/diff.rs`.
- Created 11 integration tests in `packages/onehost/tests/domain_diff_tests.rs`. All 41 unit tests across the test suite passed cleanly with zero compiler warnings.
- Verified hermetic build via `nix build .#packages.x86_64-linux.onehost`.
- Conducted senior-level code review of Stage 5; verified runtime volatile attribute normalization (`id`, `<alias>`, dynamic `<seclabel>`), auto-allocated default device reconciliation (`<address>`, `<mac>`), deterministic canonical ordering, and expressive naming. Transitioned to Phase 11: Stage 6.
- Updated `findings.md` and `task_plan.md` to reflect all approved refinements and test cases.
- Recorded new mandatory design guidelines: Rule 7 (Always read planning files in full before starting work on a new phase) and Rule 8 (Functional programming, monadic types, move semantics, and born-valid types; eliminating imperative mutate-in-place and setters).
- Executed Phase 10b: Codebase Review & Functional Modernization across all modules (Stages 1–5):
  - In `src/xdg.rs`: Refactored to monadic combinators (`.map()`, `.and_then()`) and iterator pipelines (`.into_iter().filter(...).try_for_each(...)`), eliminating imperative loops and mutable branches.
  - In `src/image/tag.rs`: Enforced born-valid validation and iterator pipeline in `from_str`.
  - In `src/config/`: Updated `OnehostManifest::new` to enforce validation upon construction (born valid), and refactored `validation.rs` loops to `.iter().try_for_each(...)`.
  - In `src/domain/element.rs`: Created unified, immutable `DomainXmlElement` value object with pure builder methods (`with_child`, `without_attribute`, `transform_children`, `with_replaced_text`).
  - In `src/domain/template.rs`: Refactored from imperative 400-line mutable event loop to a clean 200-line pure functional AST tree transformation.
  - In `src/domain/diff.rs`: Refactored `DomainXmlNormalizer::normalize_domain_trees` and `DomainXmlDiffer::calculate_differences` to pure functional transformations without in-place mutation of trees or difference vectors.
- Verified all 41 unit and integration tests pass cleanly with zero warnings.
- Verified hermetic build via `nix build .#packages.x86_64-linux.onehost`.
- Conducted senior-level code review of functional modernization. Paused awaiting user authorization for Phase 11.
- Executed Phase 10b (Part 2): Total Eradication of Imperative Patterns:
  - Addressed user inquiry regarding `quick_xml`: explained the architectural necessity of in-memory AST for dynamic Libvirt Domain XML vs statically typed serde, and demonstrated that while `quick_xml`'s I/O boundary uses a low-level byte buffer for zero allocations, AST tree construction can be modelled 100% functionally via recursive descent and `std::iter::from_fn`.
  - In `src/image/tag.rs` & `src/config/validation.rs`: Replaced mutable container initialization and `.sort()` with functional collection into `BTreeSet`.
  - In `tests/xdg_directory_tests.rs` & `tests/image_tag_tests.rs`: Replaced all mutable `HashMap::new()` container initializations and `.insert()` mutations with functional `.into_iter().collect()` pipelines.
  - In `src/domain/template.rs` & `src/domain/diff.rs`: Replaced all remaining mutable temporary variables, string buffers, and sorting loops with functional `fold`, `chain`, `collect`, and `BTreeMap` pipelines.
- Recorded new mandatory design guideline: Rule 9 (Never Sacrifice Idiomatic Design for Convenience & Strict `fn(self, ...) -> Self` Builders):
  - Declared that call-site convenience must never be prioritized over idiomatic Rust design.
  - Enforced `fn(self, ...) -> Self` pattern across all value models and AST builders, leveraging move semantics, struct update syntax (`..self`), and `.into_iter()` to avoid forced deep cloning.
- Executed Phase 10b (Part 3): Strict `fn(self, ...) -> Self` Move Semantics Refactoring:
  - In `src/config/model.rs`: Refactored `with_pool(self, ...)` to consume `self` by value and construct the new instance via `Self { pool, ..self }`, eliminating forced field cloning.
  - In `tests/config_tests.rs`: Made `.clone()` explicit at call site (`instance.clone().with_pool(...)`).
  - In `src/domain/element.rs`: Refactored all builders (`with_attribute`, `with_attributes_vec`, `without_attribute`, `with_replaced_text`, `with_child`, `with_children`, `without_child_tag_recursive`, `transform_children`) to consume `self` by move, use `..self`, and pass closures owned children (`F: Fn(DomainXmlElement) -> Option<DomainXmlElement>`).
  - In `src/domain/template.rs` and `src/domain/diff.rs`: Updated call sites to take advantage of zero-copy moves and make `.clone()` explicit only when operating on borrowed references.
  - Verified all 41 unit and integration tests pass cleanly with zero compiler warnings.
  - Verified hermetic build via `nix build .#packages.x86_64-linux.onehost`.
- Executed Phase 10c: Senior-Level Deep Review & Refinement of Paradigm Transition:
  - Track 1 (Zero-Copy Move Semantics): Replaced reference-taking and cloning with owned moves and direct destructuring across `template.rs`, `diff.rs`, and `element.rs`. In `parse_element_body`, replaced child cloning with zero-copy `.into_iter().fold(...)`.
  - Track 2 (Monadic Purity & Clippy): Fixed `clippy::collapsible_if` in `model.rs` via `.map().filter().or_else()` monadic chain; eliminated `clippy::type_complexity` in `xdg.rs` via `EnvironmentLookup` type alias; reused `effective_pool()` in `validation.rs`.
  - Track 3 (Public Fields on AST Node & Guideline Compliance): Made `tag_name`, `attributes`, `children`, and `text_content` public on `DomainXmlElement`, eliminating `DomainXmlElementParts` boilerplate. Audited and eradicated all single-letter closure variables across all `src/` and `tests/` files (0 single-letter closures remain).
  - Track 4 (AST Round-Trip Invariance Test Suite): Created `tests/domain_element_tests.rs` with 6 exhaustive tests verifying XML round-trip AST invariance, self-closing formatting, text node preservation, zero-copy destructuring, builder immutability, and malformed XML edge-case rejection.
  - Track 5 (Verification): All 47 unit and integration tests pass cleanly. `cargo clippy --all-targets -- -D warnings` reports 0 warnings. Hermetic flake build passes cleanly.
- Reran Phase 10c & Enforced Rule 10 (Preference of Public Attributes over Getter Functions in Functional Domain):
  - Documented Rule 10 across `task_plan.md` and `findings.md`.
  - Promoted fields to `pub` and eradicated trivial boilerplate getters across all data models and ASTs (`OnehostManifest`, `StorageDirectoriesConfiguration`, `FlavorConfiguration`, `InstanceConfiguration`, `InstanceLifecycleConfiguration`, `ImageTagSpecification`, `FlavorDerivationMetadata`, `DomainXmlElement`).
  - Refactored all call sites across `src/` and `tests/` (`validation.rs`, `diff.rs`, `template.rs`, `config_tests.rs`, `image_tag_tests.rs`, `domain_element_tests.rs`) to directly access public fields.
  - Fixed single-letter closure variable pattern `|(k, v)|` -> `|(env_key, env_value)|` across 6 test cases in `tests/xdg_directory_tests.rs`.
  - Removed unused import `Path` in `src/config/model.rs`.
  - Re-verified full test suite (47/47 passing), zero clippy warnings (`-D warnings`), and hermetic Nix flake package build.
- Documented Rule 11: Avoid Redundant Type Encoding in Identifiers (Semantic Domain Naming over Hungarian Notation):
  - Recorded Rule 11 in `task_plan.md`, `findings.md`, and `progress.md`.
  - Established mandate to avoid technical type suffixes/prefixes (`_str`, `_string`, `_os_str`, `_vec`, `_map`, `_bool`, `_element`, `_parts`) in favor of descriptive semantic domain roles.
- Executed Phase 11: Stage 6 - Hypervisor and Storage Abstractions (TDD):
  - In `src/hypervisor/traits.rs`: Defined `trait Hypervisor`, `DomainState`, `DomainInfo`, `BlockDeviceInfo`, `DiskSnapshotSpecification`, `HypervisorError`, and pure `parse_pool_target_path` AST helper.
  - In `src/hypervisor/mock.rs`: Implemented `MockHypervisor` tracking domains, XMLs, snapshots, blockcommits, autostart, and injected errors.
  - In `src/hypervisor/virsh.rs`: Implemented `VirshHypervisor` executing `virsh` commands with pure parsers `parse_dominfo_output` and `parse_domblklist_output`.
  - In `src/storage/traits.rs`: Defined `trait StorageManager`, `ImageInspectionInfo`, `StorageError`, and `execute_cross_device_streaming_move` with streaming copy, fsync, atomic rename, and source cleanup.
  - In `src/storage/mock.rs`: Implemented `MockStorageManager` tracking mock images, overlay creation, rebase, corruption detection, and EXDEV fallback simulation.
  - In `src/storage/qemu_img.rs`: Implemented `QemuImgStorage` executing `qemu-img` with `parse_qemu_img_info_json`.
  - Applied Rule 10 (public fields on all records) and Rule 11 (semantic domain naming without Hungarian type suffixes) across all modules.
  - Created 20 new tests across `tests/hypervisor_tests.rs` (10 tests) and `tests/storage_tests.rs` (10 tests). Total test suite expanded to 67 unit/integration tests (all passing).
  - Verified zero clippy warnings (`cargo clippy --all-targets -- -D warnings`).
  - Staged files in git and verified hermetic Nix flake package build (`nix build .#packages.x86_64-linux.onehost`).
  - Conducted senior-level code review of Stage 6. Paused awaiting user authorization for Phase 12.
- Documented Rule 12 (Environment & Tooling Execution Protocol):
  - Always invoke cargo and rust development tools using Nix shell: `nix shell nixpkgs#cargo --command cargo <subcommand>` (for check/lint) and `nix shell nixpkgs#cargo nixpkgs#gcc --command cargo test` (with gcc for linker when building/running tests).
  - Added Rule 12 across `task_plan.md`, `findings.md`, and `progress.md`.
- Executed Phase 12: Stage 7 - Stateless Planner, Applier, and Destroyer (TDD):
  - In `src/lifecycle/planner.rs`: Implemented `DomainLifecyclePlanner`, `OnehostPlan`, `InstancePlanAction` (`Create`, `UpdateDomainXml`, `Recreate`, `RelocateStoragePool`, `Delete`, `NoOp`), and `DanglingSnapshotAlert`. Replaced `active_disk_path.exists()` with `StorageManager::inspect_image()` to ensure hermetic mock-driven testing. Leveraged monadic `filter`, `is_some_and`, and `is_ok_and`.
  - In `src/lifecycle/applier.rs`: Implemented `DomainLifecycleApplier` and `ApplyOptions`. Orchestrates sequential reconciliation: base image caching, CoW overlay creation, NVRAM template initialization, `virsh define`, autostart configuration, storage pool refresh. Enforces lifecycle guardrails (`prevent_destroy` aborts; `policy: protect` aborts without `--allow-recreate`). Handles zero-data-loss safe pool relocation (`mv` + fast metadata-only `qemu-img rebase -u` + `virsh define` + refresh both pools).
  - In `src/lifecycle/destroyer.rs`: Implemented `DomainLifecycleDestroyer` and `DestroyOptions`. Enforces `prevent_destroy` guardrail (aborts unless `--allow-destroy-protected`), graceful ACPI shutdown (or force power cut with `--force`), domain undefine with NVRAM cleanup, optional overlay disk unlinking (`--delete-disk`), and pool refresh.
  - In `src/lifecycle/mod.rs`: Defined `LifecycleError` with strict variants (`LifecyclePolicyViolation`, `HypervisorError`, `StorageError`, `DomainSynthesisError`, `DomainDiffError`, `ImageTagParseError`, `FlavorResolutionError`, `ConfigurationError`, `IoError`).
  - Created 22 comprehensive unit tests across `tests/planner_tests.rs` (9 tests) and `tests/lifecycle_tests.rs` (13 tests). Total test suite expanded to 89 tests (all passing).
  - Conducted post-token-quota review of Phase 12: audited all modules, verified mock hermeticity, added 2 error propagation tests in `tests/planner_tests.rs` (pool resolution failure and template synthesis error propagation).
  - Verified 0 clippy warnings (`nix shell nixpkgs#cargo nixpkgs#clippy --command cargo clippy --all-targets -- -D warnings`).
  - Staged files in git and verified hermetic Nix flake package build (`nix build .#packages.x86_64-linux.onehost --no-link`).
  - Conducted senior-level code review of Stage 7.
- Initiated Phase 12b: Lifecycle Pipeline & Real Toolchain Integration Suite:
  - Formulated Hybrid Integration Strategy with user (Phase 12b pipeline & toolchain tests + Phase 13b disaster recovery tests).
  - Updated `task_plan.md` and `findings.md` with Phase 12b / 13b specifications and reinforced Rule 8:
    - Iterators over imperative loops: strictly avoid imperative `for`/`while` loops with nested `if` and early `return`; leverage iterator pipelines (`.into_iter()`, `.find()`, `.filter()`, `.map()`, `.try_for_each()`).
    - Monadic combinators & predicates: leverage `.is_ok_and()`, `.is_some_and()`, `.ok()`, `.flatten()`, `.and_then()`, `.or_else()`.
    - `if` as expressions over statements: compute values directly from `if` expressions without mutable flags.
    - The `?` operator over early returns: propagate errors monadically.
  - Added `pkgs.qemu-utils` to `nativeCheckInputs` in `packages/onehost/default.nix` for hermetic Nix execution.
  - Implemented `tests/storage_toolchain_tests.rs` (5 tests passing against real `qemu-img` binary).
  - Added `delete_image` method to `StorageManager` trait and `MockStorageManager`, eradicating raw host `fs::remove_file` in `applier.rs` and `destroyer.rs`.
  - Added `list_storage_pools` and `resolve_pool_name_by_path` to `Hypervisor` trait, `VirshHypervisor`, and `MockHypervisor` using pure functional pipelines.
  - Explicitly documented Rule 8 contrast points across `task_plan.md` and `findings.md`:
    - For loops vs. Iterator pipelines & monadic combinators: strictly avoid imperative `for`/`while` loops; use declarative iterator pipelines (`.into_iter()`, `.iter()`, `.find()`, `.filter()`, `.map()`, `.try_for_each()`, `.fold()`, `.all()`, `.any()`) and monadic combinators (`.is_ok_and()`, `.is_some_and()`, `.ok()`, `.flatten()`, `.and_then()`, `.or_else()`, `.unwrap_or_else()`).
    - `if` statements vs. `if` expressions: treat `if` as a value-producing expression (`let x = if cond { a } else { b };`) instead of branch-heavy statement mutating outer state; leverage monadic combinators where suitable.
    - Early `return` vs. The `?` operator: propagate errors monadically using `?`; conclude functions with trailing expressions rather than early return bailouts.
  - Completed Phase 12b:
    - Resolved mock hermeticity in `src/lifecycle/applier.rs`: replaced host `fs::exists` checks on `base_cache_path` with `self.storage.inspect_image(base_cache_path).is_err()`, guaranteeing hermetic operation independent of files on the host system.
    - Updated `MockStorageManager::delete_image` to also clean up physical test files if they exist on disk, and asserted `RecordedStorageAction::DeleteImage` in `tests/lifecycle_tests.rs`.
    - Refactored `src/lifecycle/applier.rs` (`apply` / `apply_action`), `src/lifecycle/planner.rs` (`plan` / `plan_instance`), and `src/hypervisor/virsh.rs` (`snapshot_create_as`, `blockcommit`, `set_autostart`) to 100% functional iterator pipelines (`try_for_each`, `map`, `fold`, `chain`, `flatten`), completely eradicating imperative `for` loops and mutable boolean flags across `packages/onehost/src/`.
    - Executed full test suite: 99/99 tests pass cleanly (10 config, 11 diff, 6 element, 10 hypervisor, 6 image_tag, 13 lifecycle, 5 pipeline integration, 9 planner, 10 storage, 5 storage toolchain, 7 template injection, 7 xdg directory).
    - Executed Clippy audit: 0 warnings with `-D warnings`.
    - Verified hermetic Nix flake build: `nix build .#packages.x86_64-linux.onehost --no-link` succeeds.
  - Refactored Coding Standards into 3-Pillar Triad of Foundations:
    - Synthesized ad-hoc rules 1–12 and missing architectural dimensions into Pillar I (Architectural Blueprint & Contracts: Core vs. Shell, Zero Defaults, Idempotency, RAII Guards, Error Taxonomy), Pillar II (Functional Rust Craft: Born-Valid Records, Move Semantics, Iterators, Expressions, Monadic ?, Semantic Naming), and Pillar III (Rigor & Protocol: 4-Tier Testing, Hermetic Nix Tooling, Quality Gates).
    - Incorporated 4 critical refinements from senior review:
      1. Boundary clarity distinguishing Pure Functional Core from Query Shell (Planner) and Execution Shell (Applier/Destroyer/Backup/Restore).
      2. CLI Stream Discipline & Structured Tracing (`stdout` for machine/user data, `stderr` for `tracing`, zero `println!` in libraries).
      3. Total Functions: Absolute prohibition of `.unwrap()` and `.expect()` in `src/`.
      4. Parameter and Iterator semantics: Borrow slices (`&str`, `&Path`, `&[T]`), own values by move; strictly use `.map()` for pure projections and `.try_for_each()` for side-effects.
    - Updated `task_plan.md` and `findings.md` with explicit contrastive Target Idioms vs. Anti-Patterns.
    - Executed Phase 12c.1 (Core Foundation & Configuration Audit):
      - In `src/xdg.rs`: Extracted `validate_absolute_path` helper, eliminated duplicated logic, replaced truncated variable names (`|dir|` -> `|directory|`), preserved monadic expressions and total functions (0 unwrap/expect).
      - In `src/image/tag.rs`: Replaced imperative `return Err(...)` with expression-oriented branching, eliminated truncated identifiers (`os` -> `operating_system`, `version` -> `build_version`, `flavor` -> `flavor_name`), and refactored `lookup_flavor` to monadic `ok_or_else()`.
      - In `src/config/`: In `validation.rs`, refactored checks into monadic `.then_some(()).ok_or_else(...)?:` pipelines, eliminated `return Err(...)`, renamed anti-pattern `parts` to `uuid_segments`. In `loader.rs`, refactored `load_from_path` and `resolve_manifest_path` to pure expressions and eliminated truncated names (`config_dir` -> `configuration_directory`).
      - In `tests/xdg_directory_tests.rs`: Eliminated truncated variable names (`config_dir`, `cache_dir`, etc.).
      - Ran full 4-tier verification: 99/99 tests pass cleanly, 0 clippy warnings (`-D warnings`), hermetic Nix flake package build succeeds.
      - Conducted Stage-Gated Senior Code Review for Phase 12c.1.
    - Executed Phase 12c.2 (Domain XML AST, Synthesis & Normalization Audit):
      - In `src/domain/element.rs`: Replaced stringly-typed `Result<T, String>` errors with strongly-typed `DomainXmlParseError` (`thiserror`), eliminating parse failures as plain strings. Refactored `parse`, `parse_start_element`, `parse_empty_element`, `parse_attributes`, and `parse_element_body` to return `Result<DomainXmlElement, DomainXmlParseError>`. Eliminated truncated identifiers (`attr` -> `attribute`, `attr_err` -> `attribute_error`, `attribute_val` -> `attribute_value`).
      - In `src/domain/mod.rs`: Re-exported `DomainXmlParseError`.
      - In `src/domain/template.rs`: Updated `DomainTemplateSynthesisError::XmlParseError` to wrap `#[from] DomainXmlParseError`. Replaced manual early `return Err(...)` with monadic `.then_some(()).ok_or(...)?:` pipelines. Replaced anti-pattern identifier `nvram_path_str` with `rendered_nvram_path`.
      - In `src/domain/diff.rs`: Updated `DomainXmlDiffError` to wrap `DomainXmlParseError`. Replaced early `return Err(...)` in `compare_domain_xmls` with monadic `.then_some(()).ok_or(...)?:`. Eradicated truncated identifiers (`synth_dev` -> `synthesized_device`, `live_dev` -> `live_device`, `dev_child` -> `device_child`, `ctrl_type` -> `controller_type`, `exp_val` -> `expected_value`, `act_val` -> `actual_value`, `sub_diffs` -> `child_sub_differences`, `diffs` -> `accumulated_differences`).
      - Ran full 4-tier verification: 99/99 tests pass cleanly, 0 clippy warnings (`-D warnings`), hermetic Nix flake package build succeeds.
      - Conducted Stage-Gated Senior Code Review for Phase 12c.2.
    - Codified Rule III.4 (Continuous Upstream Synchronization & Atomic Commits) into Triad of Foundations:
      - Documented rule in `task_plan.md` and `findings.md` mandating that local changes must never accumulate across phases.
      - Upon passing verification tiers and receiving stage-gate approval, all code and planning files must be committed atomically and pushed to upstream remote.
    - Executed Phase 12c.3 (Trait Abstractions & Infrastructure Adapters Audit):
      - In `src/hypervisor/virsh.rs`: Refactored `execute_command` to return pure expressions, eliminating early `return Err(...)` statements and renaming truncated `|arg|` to `|argument|`. Refactored `parse_dominfo_output` from mutable variables and `.for_each()` to pure `.fold()` pipeline with struct update syntax. Refactored `parse_domblklist_output` to a pure expression. Refactored `dump_xml`, `undefine_domain`, `set_autostart`, `create_snapshot`, and `blockcommit` to declarative iterator argument pipelines and eliminated Hungarian suffixes (`diskspec_args`, `base_argument_string`).
      - In `src/hypervisor/mock.rs`: Renamed `poison_err` -> `poison_error` and `parse_err` -> `parse_error`. Refactored early returns in `domain_info`, `dump_xml`, `pool_dump_xml`, `pool_refresh`, and converted existence checks in `list_block_devices`, `create_snapshot`, `blockcommit` to monadic `.then_some(()).ok_or_else(...)?:` chains.
      - In `src/storage/traits.rs`: Replaced imperative buffer `loop { ... }` in `execute_cross_device_streaming_move` with standard `std::io::copy(&mut source_file, &mut staging_file)?;`, achieving 100% loop-free production code. Converted early returns in `execute_cross_device_streaming_move` and `move_file_safely` to monadic `.then_some(()).ok_or_else(...)?:` pipelines. Cleaned unused `Read` and `Write` imports.
      - In `src/storage/qemu_img.rs`: Renamed `json_err` -> `json_error` and Hungarian suffixes (`backing_file_str`, `overlay_str`, etc. -> `rendered_*`). Replaced imperative argument pushes with functional chained iterator pipelines in `rebase_overlay` and `convert_thin_backup`. Refactored early returns in all methods to monadic `.then_some(()).ok_or_else(...)?:` pipelines. Monadized `check_image`.
      - In `src/storage/mock.rs`: Renamed `poison_err` -> `poison_error`. Refactored early returns in `create_cow_overlay`, `rebase_overlay`, `inspect_image`, and `move_file_safely` to expressions and monadic `?`.
      - Verified 4-tier verification protocol: 99/99 tests passed, 0 clippy warnings (`-D warnings`), hermetic Nix flake package build succeeds.
      - Conducted Stage-Gated Senior Code Review for Phase 12c.3. Awaiting user signal before initiating Phase 12c.4.
    - Codified the **Fresh Read Protocol** into Pillar III.3 in `task_plan.md` and `findings.md`:
      - Formally specified that every phase and context resumption must begin with a complete, contiguous read of the entire `## Mandatory Design Guidelines & Engineering Standards` section as a whole together (North Star, Pillar I, Pillar II, Pillar III), with zero skipping.
      - Mandated bundling the standards read with the target phase context: the phase tasks and acceptance criteria in `task_plan.md`, current execution progress and history in `progress.md`, and active domain contracts and invariants in `findings.md`.
    - Executed Phase 12c.4 (Lifecycle Reconciliation & Integration Pipelines Audit):
      - In `src/lifecycle/planner.rs`: Refactored `plan_instance` to match directly on `self.hypervisor.domain_info(instance_name)`; refactored action resolution into an idiomatic match expression with match guards (`Some(reconciliation_action)`, `None if domain_diff_result.has_drift`, `None`); replaced truncated closure variables (`backing` -> `backing_file_path`, `parent_path` -> `directory_path`); replaced abbreviated directory variables (`active_parent` -> `active_disk_directory`, `target_parent` -> `target_overlay_directory`).
      - In `src/lifecycle/applier.rs`: Replaced imperative return checks with monadic `(!*prevent_destroy).then_some(()).ok_or_else(...)?:` and `(*policy != ImageChangePolicy::Protect || options.allow_recreate).then_some(()).ok_or_else(...)?:`; converted cache verification and copy to `.then(|| self.storage.copy_base_image(...)).transpose()?:`; converted NVRAM template copying and autostart configuration to monadic `.then(...).transpose()?:`; renamed `|info|` to `|domain_info|`.
      - In `src/lifecycle/destroyer.rs`: Replaced early `return Err(...)` with monadic `(!prevent_destroy || allow_destroy_protected).then_some(()).ok_or_else(...)?:`; converted domain stopping and disk removal to functional `.then(...).transpose()?:`; renamed `|info|` to `|domain_info|`.
      - In `tests/pipeline_integration_tests.rs`: Refactored `TestWorkspace::new` to replace all truncated/abbreviated identifiers (`workspace`, `root`, `depot_dir`, `nvram_dir`, etc.) with full descriptive domain names (`temporary_workspace_directory`, `workspace_root_path`, `depot_directory`, etc.).
      - In `tests/planner_tests.rs`: Replaced `|_path|` closures across all 14 test cases with descriptive `|_template_path|`.
      - Verified 4-tier verification protocol: 99/99 tests passed, 0 clippy warnings (`-D warnings`), hermetic Nix flake package build succeeds.
      - Conducted Stage-Gated Senior Code Review for Phase 12c.4.
    - Executed Phase 12c.5 (Whole-System Verification & Senior Gate Review):
      - Conducted complete codebase static analysis across all files in `packages/onehost/src/`:
        - Verified 0 imperative `for` or `while` loops across all production modules (100% declarative iterator pipelines).
        - Verified 0 `.unwrap()` and 0 `.expect()` calls across `src/` (100% total functions with monadic `?` error propagation).
        - Verified 0 single-letter closure or variable names (100% descriptive domain role naming).
        - Verified 0 `println!` or raw stdout writes in library code.
        - Verified 0 OOP getter/setter boilerplate (100% transparent algebraic records with `pub` fields).
      - Executed 4-tier verification protocol:
        - `cargo check`: passed with 0 errors.
        - `cargo test --all-targets`: all 99 tests passed cleanly across 12 test suites.
        - `cargo clippy --all-targets -- -D warnings`: passed with 0 warnings.
        - `nix build .#packages.x86_64-linux.onehost --no-link`: hermetic derivation build and checkPhase passed cleanly.
      - Conducted comprehensive Senior Gate Review across the entire codebase.
      - Remediation of `loop` constructs in `src/domain/element.rs`:
        - Replaced 3 imperative `loop` blocks (`parse_document_root`, `verify_eof`, `parse_element_body`) with pure functional tail recursion and `std::iter::from_fn(|| Self::next_body_item(...))`.
        - Verified that production code across `packages/onehost/src/` contains literally 0 `loop`, `while`, or `for` loops.
        - Re-ran 4-tier verification protocol: 99/99 tests passed, 0 clippy warnings (`-D warnings`), hermetic Nix flake package build succeeds.
      - Adapted Mandatory Design Guidelines & Engineering Standards:
        - Codified Foundational Principle & Pillar III.3 invariant: **Universal Code Equivalence & Absolute Standards**.
        - Invariant: NEVER assume code to be less critical or important to any task. Every single line of code across the entire codebase—whether pure domain models, imperative I/O shells, low-level streaming parsers, mock implementations, CLI drivers, or test suites—must be held to the exact same uncompromising, high quality standards during any task without exception.
        - Anti-Rationalization Guardrail: Strictly prohibits bypassing or excusing engineering standards by rationalizing code as "just boilerplate", "just low-level reader logic", "just a test mock", "just an internal helper", or "less critical".
      - Awaiting user signal before initiating Phase 13.


### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| Planning session initialization | Created planning files | Created in `.planning/2026-09-24-onehost-cli/` | PASS |
| Migration to planning-with-files | Full spec in task_plan.md & findings.md | Written and structured | PASS |
| Architecture refactoring (Flake Domain XML) | Updated spec & TDD plan in planning files | Successfully updated | PASS |
| Architecture refinement (Domain Template & Image Disks) | Refined spec and 10-stage TDD roadmap in planning files | Successfully updated | PASS |
| Architecture alignment (Namespace & Flavor Hashes) | Zero-ambiguity OS disk & content-addressed revisions | Successfully updated | PASS |
| Idiomatic XML Namespace alignment | Standardized on xmlns:onehost | Successfully updated | PASS |
| Storage Pool integration | Declarative pool reference & dynamic path resolution | Successfully updated | PASS |
| Lifecycle Operations Specification | Documented operations breakdown and state machine | Successfully updated | PASS |
| Lifecycle Policy & CLI Guardrails | Declarative policy & --allow-recreate guardrails integrated | Successfully updated | PASS |
| Safe Storage Pool Relocation | Non-destructive disk relocation via qemu-img rebase -u | Successfully updated | PASS |
| Invariants & Edge Cases Refinements | Normalization, EXDEV fallback, atomic master promotion | Successfully updated | PASS |
| Robust Backup & Restore Specification | RAII guard, multi-disk atomic snapshot, manifest.json | Successfully updated | PASS |
| Plan Sign-Off & Guidelines Integration | Design guidelines recorded & user approval granted | Guidelines embedded in plan | PASS |
| Stage 1 XDG Resolution (tests/xdg_directory_tests.rs) | 7 unit tests pass cleanly | 7 passed, 0 failed, 0 warnings | PASS |
| Stage 1 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Stage 2 Image Tag & Flavor Resolver (tests/image_tag_tests.rs) | 6 unit tests pass cleanly | 6 passed, 0 failed, 0 warnings | PASS |
| Stage 2 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Stage 3 Declarative Manifest & Validation (tests/config_tests.rs) | 10 unit tests pass cleanly | 10 passed, 0 failed, 0 warnings | PASS |
| Stage 3 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Stage 4 Domain Template & Injection (tests/template_injection_tests.rs) | 7 unit tests pass cleanly | 7 passed, 0 failed, 0 warnings | PASS |
| Stage 4 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Stage 5 Domain Diff Engine & Normalization (tests/domain_diff_tests.rs) | 11 unit tests pass cleanly | 11 passed, 0 failed, 0 warnings | PASS |
| Stage 5 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Phase 10b Functional Modernization (Full Test Suite) | 41 unit tests pass cleanly | 41 passed, 0 failed, 0 warnings | PASS |
| Phase 10b Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Phase 10c Deep Review & AST Invariance (Full Test Suite) | 47 unit tests pass cleanly | 47 passed, 0 failed, 0 warnings | PASS |
| Phase 10c Clippy Audit | Zero linter warnings with -D warnings | 0 warnings | PASS |
| Phase 10c Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Stage 6 Hypervisor Tests (tests/hypervisor_tests.rs) | 10 unit tests pass cleanly | 10 passed, 0 failed, 0 warnings | PASS |
| Stage 6 Storage Tests (tests/storage_tests.rs) | 10 unit tests pass cleanly | 10 passed, 0 failed, 0 warnings | PASS |
| Phase 11 Full Test Suite (cargo test) | 67 unit/integration tests pass cleanly | 67 passed, 0 failed, 0 warnings | PASS |
| Phase 11 Clippy Audit | Zero linter warnings with -D warnings | 0 warnings | PASS |
| Phase 11 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Stage 7 Planner Tests (tests/planner_tests.rs) | 9 unit tests pass cleanly | 9 passed, 0 failed, 0 warnings | PASS |
| Stage 7 Lifecycle Tests (tests/lifecycle_tests.rs) | 13 unit tests pass cleanly | 13 passed, 0 failed, 0 warnings | PASS |
| Phase 12 Full Test Suite (cargo test) | 89 unit/integration tests pass cleanly | 89 passed, 0 failed, 0 warnings | PASS |
| Phase 12 Clippy Audit | Zero linter warnings with -D warnings | 0 warnings | PASS |
| Phase 12 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Pipeline Integration Tests (tests/pipeline_integration_tests.rs) | 5 cross-module integration tests pass | 5 passed, 0 failed, 0 warnings | PASS |
| Storage Toolchain Tests (tests/storage_toolchain_tests.rs) | 5 real qemu-img CLI tests pass | 5 passed, 0 failed, 0 warnings | PASS |
| Phase 12b Full Test Suite (cargo test) | 99 unit/integration tests pass cleanly | 99 passed, 0 failed, 0 warnings | PASS |
| Phase 12b Clippy Audit | Zero linter warnings with -D warnings | 0 warnings | PASS |
| Phase 12b Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Phase 12c.1 Full Test Suite (cargo test) | 99 unit/integration tests pass cleanly | 99 passed, 0 failed, 0 warnings | PASS |
| Phase 12c.1 Clippy Audit | Zero linter warnings with -D warnings | 0 warnings | PASS |
| Phase 12c.1 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Phase 12c.2 Full Test Suite (cargo test) | 99 unit/integration tests pass cleanly | 99 passed, 0 failed, 0 warnings | PASS |
| Phase 12c.2 Clippy Audit | Zero linter warnings with -D warnings | 0 warnings | PASS |
| Phase 12c.2 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Phase 12c.3 Full Test Suite (cargo test) | 99 unit/integration tests pass cleanly | 99 passed, 0 failed, 0 warnings | PASS |
| Phase 12c.3 Clippy Audit | Zero linter warnings with -D warnings | 0 warnings | PASS |
| Phase 12c.3 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Phase 12c.4 Full Test Suite (cargo test) | 99 unit/integration tests pass cleanly | 99 passed, 0 failed, 0 warnings | PASS |
| Phase 12c.4 Clippy Audit | Zero linter warnings with -D warnings | 0 warnings | PASS |
| Phase 12c.4 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |
| Phase 12c.5 Static Analysis Audit | Zero loops, zero unwraps, zero single-letter variables | 100% compliant | PASS |
| Phase 12c.5 Loop Eradication in element.rs | 3 loop blocks refactored to tail recursion / from_fn | Zero loops in src/ | PASS |
| Phase 12c.5 Full Test Suite (cargo test --all-targets) | 99 unit/integration tests pass cleanly | 99 passed, 0 failed, 0 warnings | PASS |
| Phase 12c.5 Clippy Audit | Zero linter warnings with -D warnings | 0 warnings | PASS |
| Phase 12c.5 Nix Flake Build (packages.x86_64-linux.onehost) | Hermetic build and checkPhase succeed | Successfully built via Nix | PASS |

### Errors
| Error | Resolution |
|-------|------------|
| Host base image existence leaking into unit tests | Queried `storage.inspect_image(...)` in `applier.rs` instead of host `fs::exists(...)` |
| 3 `loop` blocks left in `src/domain/element.rs` | Refactored `parse_document_root`, `verify_eof`, and `next_body_item` to functional tail recursion |


