# Task Plan: Declarative Flake Module Evaluator (Rust-Inspired Namespaces)

## Goal
Replace the filesystem-crawling custom module evaluator in `flake.nix` with a declarative, monadic module builder system (`createFlakeModule`, `pubMod`, `mod`) supporting strictly `self`, `super`, and `flake` scopes (no aliases), accepting function values in `createFlakeModule`, resolving the `packages.onehost` architecture collision and updating all nix files across the flake.

## Next Step
Phase 11 complete. GitHub Actions CI running green on `tarberd/flake-modules`. Await user feedback or new task directives.

## Current Phase
Phase 11: TDD & CI Test Infrastructure (`tests/` Subflake & `nix-unit`) (Complete)

## Phases

### Phase 1: Requirements & Discovery
- [x] Analyze current `flake.nix` evaluator, `readDirSafe`, `getNixTree`, and `evaluateAndFlatten`
- [x] Diagnose `packages.onehost` error during `nix flake check`
- [x] Audit all `.nix` files, imports, functors, and cross-references in the repository
- [x] Document findings and design decisions in `findings.md`
- **Status:** complete

### Phase 2: Design & Planning
- [x] Design monadic module builder structure (`{ privateModules; publicModules; content; }`)
- [x] Design functor combinators (`pubMod`, `mod`, `createFlakeModule`) allowing natural chaining without nested parentheses
- [x] Design `self`, `super`, and `flake` scoping rules (strictly no aliases; eliminate `selfModule`/`superModule`)
- [x] Ensure `createFlakeModule` accepts function values (e.g. `{}:{}`) as well as attribute sets and empty `{}`
- [x] Verify Nix lazy evaluation mechanics with tests to prevent infinite recursion
- [x] Formulate comprehensive migration plan for all intermediate directories and leaf modules
- [x] Align with user requirements
- **Status:** complete

### Phase 3: Evaluator Engine Implementation in `flake.nix`
- [x] Implement `createFlakeModule`, `pubMod`, and `mod` monadic combinators
- [x] Implement `resolveSubmodule` (resolves name to `.nix` file or `/default.nix`)
- [x] Implement `evalModule` passing strictly `self`, `super`, `flake`, and combinators to module functions
- [x] Support function values in `content` by converting to functor attrsets preserving `__functionArgs`, `key`, and `_file`
- [x] Implement root flake evaluator declaring private namespaces and public outputs via the monadic builder
- [x] Preserve `declareNixosModule` functionality
- **Status:** complete

### Phase 4: Module Declarations & Package Integration across Codebase
- [x] Update `packages/default.nix` to use `mod "onehost" createFlakeModule { ... }` and re-export `x86_64-linux.onehost = self.onehost;`
- [x] Implement `packages/onehost/default.nix` using `pkgs.rustPlatform.buildRustPackage` and `createFlakeModule`
- [x] Track `packages/onehost/Cargo.lock` in git
- [x] Update `gameservers/default.nix` (replace `selfModule` with `self`, add `mod "config" mod "instances" mod "images" createFlakeModule { ... }`)
- [x] Update `gameservers/config.nix` (replace `superModule` with `super`)
- [x] Create `gameservers/images/default.nix` (`pubMod "palworld" createFlakeModule {}`)
- [x] Create `middle-earth/default.nix` (`pubMod "hosts" pubMod "users" createFlakeModule {}`)
- [x] Create `middle-earth/hosts/default.nix` (`pubMod "gandalf" pubMod "sauron" createFlakeModule {}`)
- [x] Create `middle-earth/hosts/gandalf/default.nix` (`mod "hardware-configuration" mod "storage" mod "network" mod "backup" pubMod "configuration" pubMod "virtualization" createFlakeModule {}`)
- [x] Update `middle-earth/hosts/gandalf/configuration.nix` (replace `superModule` with `super`)
- [x] Create `middle-earth/hosts/gandalf/virtualization/default.nix` (`pubMod "kvm" pubMod "container" pubMod "instances" pubMod "images" createFlakeModule {}`)
- [x] Create `middle-earth/hosts/gandalf/virtualization/images/default.nix` (`mod "windows-versions" pubMod "windows" createFlakeModule {}`)
- [x] Update `middle-earth/hosts/gandalf/virtualization/images/windows.nix` (replace `superModule` with `super`)
- [x] Create `middle-earth/hosts/sauron/default.nix` (`mod "disko-config" pubMod "configuration" createFlakeModule {}`)
- [x] Update `middle-earth/hosts/sauron/configuration.nix` (replace `superModule` with `super`)
- [x] Update `middle-earth/users/default.nix` (`pubMod "root" pubMod "tarberd" createFlakeModule ...`)
- [x] Create `firewalld/default.nix` (`pubMod "firewalld-policies" createFlakeModule {}`)
- **Status:** complete

### Phase 5: Verification & Testing
- [x] Test evaluation with `nix flake check`
- [x] Test flake outputs with `nix flake show` (verify clean standard schema)
- [x] Test package build: `nix build .#packages.x86_64-linux.onehost`
- [x] Test apps: `nix eval .#apps.x86_64-linux.onehost.program`
- [x] Test NixOS configurations: `nix eval .#nixosConfigurations.gandalf.config.system.build.toplevel.drvPath`
- [x] Test NixOS configurations: `nix eval .#nixosConfigurations.sauron.config.system.build.toplevel.drvPath`
- **Status:** complete

### Phase 6: Evaluator Deep Revision & Hardening
- [x] 6.1: Dead Code Removal
  - [x] Eliminate redundant `actualPath` directory check in `evalModule` (rely on `resolveSubmodule`)
  - [x] Remove dead `builtins.isPath` check in `resolveSubmodule` and enforce string names
  - [x] Consolidate `createFlakeModule` terminal constructor into `builder [] [] createFlakeModule`
- [x] 6.2: Ambiguity & Implicit Default Removal
  - [x] Forbid duplicate submodule declarations: catch `mod "foo" mod "foo"`, `pubMod "foo" pubMod "foo"`, and `pubMod "foo" mod "foo"` by enforcing strict uniqueness across `priv ++ pub`
  - [x] Add early type validation in `pubMod` and `mod` to catch non-string arguments at declaration
  - [x] Enforce strict type validation in `declareNixosModuleFor` (throw on invalid types instead of silent fallback)
  - [x] Add collision detection between module `content` attribute keys and declared submodule names in `mergeSubmodules`
  - [x] Make root output merging in `evalFlake` use `mergeSubmodules` uniformly
- [x] 6.3: Code Hygiene & Structural Refactoring
  - [x] Rename functor self parameter in `declareNixosModuleFor` to `_functorSelf` to avoid scope confusion
  - [x] Replace unqualified `dirOf` with `builtins.dirOf`
  - [x] Unify `normalizeModuleList` and `normalizeRootList` into a single `evalSubmodules` helper
- [x] 6.4: Verification & Negative Test Validation
  - [x] Verify standard `nix flake check` and `nix flake show` pass cleanly
  - [x] Test failure cases: duplicate declarations (`mod`/`mod`, `pub`/`pub`, `pub`/`mod`), non-string module name, content attribute collision
- **Status:** complete

### Phase 7: Standalone Flake Extraction & Reusability Design
- [x] 7.1: Evaluate Evaluator Independence
  - [x] Audit all functions in `flake.nix` evaluator to verify 100% zero-dependency pure Nix implementation
- [x] 7.2: Standalone Flake Architecture & Export Schema
  - [x] Design standalone flake outputs (`lib.evalFlake`, `lib.createFlakeModule`, `lib.pubMod`, `lib.mod`, `lib.declareNixosModuleFor`, `lib.mkFlake`)
- [x] 7.3: Consumer Integration Blueprint
  - [x] Document repository setup, flake inputs integration, documentation/examples for other users, and how `middle-earth` will consume the library
  - [x] Created `/home/tarberd/flake-modules` repository with `lib/`, `template/`, and `README.md`
  - [x] Pushed to `git@github.com:tarberd/flake-modules.git`
  - [x] Integrated `inputs.flake-modules` into `middle-earth/flake.nix` and locked with `nix flake lock`
  - [x] Verified `nix flake check`, `nix flake show`, package builds, and NixOS configurations
- **Status:** complete

### Phase 8: Opinionated Entrypoint (`flake-modules.nix`) & Directory Contract
- [x] 8.1: Design & Implement Opinionated Entrypoint in `flake-modules`
  - [x] Eliminate inline `createFlakeModule` call in consuming `flake.nix`
  - [x] Implement directory-only argument in `evalFlake rootDir inputs`: looks up `rootDir + "/flake-modules.nix"` and throws an error if it does not exist
  - [x] Reject file paths with explicit error if user attempts to pass a file instead of a directory
  - [x] Evaluate `flake-modules.nix` as a homogeneous first-class module receiving standard `{ createFlakeModule, pubMod, mod, declareNixosModule, self, super, flake, ... }`
  - [x] Support both curried invocation (`evalFlake ./. inputs`) and attrset invocation (`evalFlake { inherit inputs; rootDir = ./.; }`)
  - [x] Update `template/flake-modules.nix` and `template/flake.nix`
  - [x] Update `README.md` with the new entrypoint documentation
  - [x] Commit and push changes to `main` on GitHub (`tarberd/flake-modules`)
- [x] 8.2: Consumer Integration in `middle-earth`
  - [x] Create `/home/tarberd/middle-earth/flake-modules.nix` declaring all root namespaces and outputs
  - [x] Simplify `/home/tarberd/middle-earth/flake.nix` outputs to a single line: `outputs = inputs@{ flake-modules, ... }: flake-modules.lib.evalFlake ./. inputs;`
  - [x] Update `flake.lock` with `nix flake update flake-modules`
  - [x] Verify `nix flake check`, `nix flake show`, and NixOS toplevel derivation evaluation (`nixosConfigurations.gandalf`)
- **Status:** complete

### Phase 9: Monadic Visibility Modifier (`pub mod`)
- [x] 9.1: Evaluator Implementation in `flake-modules`
  - [x] Implement `pub` functor modifier in `lib/default.nix` that accepts `mod` and returns a module builder with target added to `publicModules`
  - [x] Handle standalone/start-of-chain (`pub mod "foo"`) and chained (`mod "foo" pub mod "bar"`) states
  - [x] Add strict validation: throw descriptive error if anything other than `mod` follows `pub` (e.g. `pub "foo"`)
  - [x] Completely remove `pubMod` (no backward compatibility; strictly require `pub mod`)
  - [x] Pass `pub` in `moduleArgs` to all module functions (and remove `pubMod` from `moduleArgs` and `lib` exports)
  - [x] Update `template/flake-modules.nix` and `README.md`
  - [x] Run negative and positive tests on `flake-modules`
  - [x] Commit and push to `main` on GitHub (`tarberd/flake-modules`)
- [x] 9.2: Migration & Verification in `middle-earth`
  - [x] Update `middle-earth/flake.lock` via `nix flake update flake-modules`
  - [x] Migrate all module files in `middle-earth` from `pubMod` to `pub mod`:
    - `flake-modules.nix`
    - `middle-earth/default.nix`
    - `middle-earth/hosts/default.nix`
    - `middle-earth/hosts/gandalf/default.nix`
    - `middle-earth/hosts/gandalf/virtualization/default.nix`
    - `middle-earth/hosts/gandalf/virtualization/images/default.nix`
    - `middle-earth/hosts/sauron/default.nix`
    - `middle-earth/users/default.nix`
    - `gameservers/images/default.nix`
    - `firewalld/default.nix`
  - [x] Verify zero remaining `pubMod` occurrences across the codebase
  - [x] Run verification suite: `nix flake check`, `nix flake show`, building packages, and evaluating NixOS configurations
- **Status:** complete

### Phase 10: Canonical NixOS Module Protocol & Reserved Keyword Hardening
- [x] 10.1: Canonical Key & File Path Alignment in `declareNixosModuleFor`
  - [x] Replace `hashString "sha256"` with canonical string file path `toString filePath` to ensure full compatibility with NixOS `disabledModules` path matching
  - [x] Respect author-defined `module.key` and `module._file` if explicitly declared
  - [x] Stamp `key` and `_file` on both the returned module configuration and the outer functor object to support `disabledModules = [ flake.someModule ]`
- [x] 10.2: Functor Isolation for Submodule Containment
  - [x] Lift both functions and attribute sets to functors in `declareNixosModuleFor` so that attached submodules (`// subs`) live exclusively on the outer functor and never leak into NixOS's `unifyModuleSyntax` `attrsToRemove` validation
- [x] 10.3: Ecosystem & Negative Test Suite
  - [x] Test multiple identical module imports across hosts and specialisations (deduplication)
  - [x] Test `disabledModules` using path string, path literal, and direct module reference
  - [x] Test reserved attributes: `options`, `config`, `_class`, `meta`, `freeformType`, and `disabledModules`
  - [x] Update `flake-modules` and `middle-earth`
- **Status:** complete

### Phase 11: TDD & CI Test Infrastructure (`tests/` Subflake & `nix-unit`)
- [x] 11.1: Design & Initialize `tests/` Subflake
  - [x] Create `flake-modules/tests/flake.nix` consuming parent via `path:..` and `nixpkgs` + `nix-unit`
  - [x] Expose `checks.<system>` derivations for `nix flake check`
- [x] 11.2: Minimal Unit Test Suite (`tests/test_lib.nix`)
  - [x] Test combinators & syntax: `pub mod`, `mod`, chaining, empty content, function content
  - [x] Test error handling with exact error messages: duplicate submodules (`mod`/`mod`, `pub`/`pub`, `pub`/`mod`), non-string names, missing `mod` after `pub`
  - [x] Test submodule resolution: file vs directory resolution, missing submodule, ambiguous collision
  - [x] Test content merging & collision detection: empty submodules, attrsets, functors, attribute key collisions
  - [x] Test NixOS module protocol: canonical `key` and `_file`, author-defined overrides, reserved keywords, invalid type error
  - [x] Test NixOS module integration: multiple import deduplication, `disabledModules` path string & direct ref, submodule containment
  - [x] Test root flake evaluator: missing `flake-modules.nix`, file path rejection, privacy enforcement, lexical scopes (`self`, `super`, `flake`)
  - [x] Test exported starter template: evaluates cleanly via `evalFlake`
- [x] 11.3: GitHub Actions CI Workflow (`.github/workflows/ci.yml`)
  - [x] Enforce zero-dependency invariant on root flake (`inputs = {}`)
  - [x] Run `nix flake check ./tests` executing the complete unit test suite and NixOS matrix in CI
- [x] 11.4: Verification & Git Integration
  - [x] Verify test suite passes locally with `nix flake check ./tests` (34/34 tests pass)
  - [x] Verify instant TDD execution with `nix-unit`
  - [x] Commit and push to GitHub `tarberd/flake-modules`
- [x] 11.5: Test & Iterate on GitHub Actions CI Execution Results
  - [x] Inspect remote GitHub Actions workflow run for commit `3ef3df3` using `gh` CLI
  - [x] Diagnose and resolve action repository casing (`DeterminateSystems`), permissions (`id-token`, `contents`), and FlakeHub login warnings
  - [x] Upgrade `actions/checkout@v4` to `@v6` (Node 24 runtime) and configure `determinate: false` on `nix-installer-action` to eliminate all 4 warning annotations
  - [x] Pin DeterminateSystems actions to major version tags (`nix-installer-action@v23`, `magic-nix-cache-action@v15`, `checkout@v6`) to prevent sudden breakage from mutable `@main`
  - [x] Verify green status with 0 errors and 0 warnings in GitHub Actions (runs `35995528948`, `35996287235`, `35997909719`, and `35999015644` succeeded)
- **Status:** complete

## Decisions Made
| Decision | Rationale |
|----------|-----------|
| Monadic structure `{ privateModules; publicModules; content; }` | Separates module metadata completely from content, preventing any metadata pollution in flake outputs. |
| Combinators `pubMod`, `mod`, and `createFlakeModule` as callable attribute sets (`__functor`) | Enables natural unparenthesized left-to-right chaining in Nix without nesting parentheses. |
| Scopes strictly `self`, `super`, and `flake` (no aliases) | Enforces clean, idiomatic naming mirroring Rust's `self`, `super`, and `crate`. Existing `selfModule` and `superModule` usages are replaced. |
| Support function values in `createFlakeModule` (e.g. `{}:{}`) | Converts functions into functors preserving `__functionArgs` and file metadata, allowing submodules to be attached while remaining callable as NixOS/Terranix modules. |
| Use `{}` for empty namespace content | In Nix, `()` is an invalid syntax error; `{}` represents empty attribute set. |
| Automatic path lookup (`.nix` or `/default.nix`) | Matches Rust module lookup (`foo.rs` or `foo/mod.rs`), minimizing boilerplate. |
| Flake root exports only `publicModules` | Prevents internal namespaces from polluting flake schema while keeping them accessible via `flake` or function arguments. |
| Standalone repository `flake-modules` | Decouples evaluator library completely from `middle-earth`, allowing zero-dependency reuse across other Nix flakes. |
| Opinionated entrypoint `flake-modules.nix` | Removes inline `createFlakeModule` in `flake.nix`, ensuring 100% syntactic and conceptual homogeneity across every module in the project. |
| Directory argument contract (`evalFlake ./. inputs`) | `evalFlake` strictly receives the root directory where `flake-modules.nix` must reside, throwing an informative error if absent or if a file path is passed. |
| Monadic visibility modifier `pub mod "foo"` (no backward compatibility) | Replaces `pubMod` entirely with the modifier `pub` applied to `mod`, producing an exact 1:1 match with Rust's `pub mod foo;` in unparenthesized Nix syntax. `pubMod` is completely removed without backward compatibility. |
| Canonical NixOS module protocol (`key = toString filePath;`) | Adheres directly to NixOS `lib/modules.nix` protocol by using string file paths rather than hashes, ensuring full compatibility with `disabledModules` and error reporting. |
| Functor isolation for NixOS modules | Lifts module attribute sets into functors so attached submodules reside on the outer functor set and do not trigger NixOS `attrsToRemove` validation errors. |
| Pin CI actions to major version tags (`@v23`, `@v15`, `@v6`) | Prevents surprise breakages and non-deterministic CI failures caused by mutable upstream `@main` branches while still receiving patch fixes. |

## Errors Encountered
| Error | Resolution |
|-------|------------|
| `packages.onehost` is not an architecture during `nix flake check` | Evaluator auto-flattened `packages/onehost` into `packages`. Solved by private module scoping. |
| Infinite recursion when merging function args with `// submodules` | Evaluated submodules must be accessed through a structured attribute (`self.submod`), not merged into the function argument set before calling. |
| Nix syntax error on `()` | In Nix, `()` is illegal syntax. Use `{}` or `null` for empty content. |
| Duplicate option declaration for `middle-earth.userRoles` | Functors created from function `content` must return `key` and `_file` so NixOS module system deduplicates duplicate imports. |
| `'key' is not a valid system type` on `apps` | Injected `key` and `_file` only into function values / functors, never into plain attribute sets. |
| Pure-eval path access restriction in CLI tests | Pure Nix eval cannot access paths outside store; used `--impure` for ad-hoc path test expressions while flakes remain fully pure. |
