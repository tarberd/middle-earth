# Progress Log

## Session: 2026-09-23

### Current Status
- **Phase:** 5 - Verification & Testing
- **Status:** Complete
- **Active Plan:** `.planning/2026-09-23-declarative-flake-module-evaluator/`

### Actions Taken
- Implemented monadic combinators (`createFlakeModule`, `pubMod`, `mod`) in `flake.nix`.
- Added support for function values (e.g. `{}:{}`) in `createFlakeModule` by converting functions into functors carrying `__functionArgs`, `key`, and `_file` for seamless NixOS module deduplication.
- Restricted scopes strictly to `self`, `super`, and `flake` (no aliases).
- Updated all files using old `selfModule` and `superModule` to use `self` and `super`.
- Created module declarations (`default.nix`) across all intermediate directories in `middle-earth`, `hosts`, `gandalf`, `virtualization`, `images`, `sauron`, `gameservers`, `firewalld`, and `packages`.
- Updated `packages/default.nix` with `mod "onehost"` and re-exported `x86_64-linux.onehost = self.onehost;`.
- Implemented `packages/onehost/default.nix` and built package using `pkgs.rustPlatform.buildRustPackage`.
- Verified `nix flake check`, `nix flake show`, building `packages.x86_64-linux.onehost`, evaluating all apps, and evaluating NixOS configurations for `gandalf` and `sauron`.
- Performed deep revision of evaluator: removed dead code (`actualPath`, `isPath`), added strict duplicate protection (`elem name (priv ++ pub)`), early type validation, and content collision protection.
- Initialized standalone `flake-modules` repo at `/home/tarberd/flake-modules` cloned from `git@github.com:tarberd/flake-modules.git`.
- Implemented `lib/default.nix`, `flake.nix`, `template/`, and `README.md` in `flake-modules` and pushed to GitHub.
- Integrated `inputs.flake-modules` into `middle-earth/flake.nix`, reducing `middle-earth/flake.nix` from 248 lines to 60 lines.
- Verified end-to-end evaluation, package builds, and NixOS configuration evaluation in `middle-earth`.

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| `nix flake check` | All checks pass with 0 errors | All checks passed | Passed |
| `nix flake show` | Only standard flake outputs | Clean tree with `apps`, `packages`, `nixosConfigurations` | Passed |
| `nix build .#packages.x86_64-linux.onehost` | Builds Rust package derivation | Successfully built | Passed |
| `apps.x86_64-linux.onehost.program` | Points to onehost binary in nix store | `/nix/store/...-onehost-0.1.0/bin/onehost` | Passed |
| `nixosConfigurations.gandalf` | Evaluates toplevel derivation | Evaluated cleanly | Passed |
| `nixosConfigurations.sauron` | Evaluates toplevel derivation | Evaluated cleanly | Passed |
| Zero `selfModule`/`superModule` left | No references in repository | Grep returned 0 matches | Passed |
| Evaluator Hardening (`nix flake check`) | All checks pass | Clean evaluation | Passed |
| Duplicate declaration test (`mod` / `mod`) | Throws Duplicate submodule declaration | Threw expected error | Passed |
| Visibility collision test (`pubMod` / `mod`) | Throws Duplicate submodule declaration | Threw expected error | Passed |
| Type validation test (`pubMod 123`) | Throws `expects a string module name` | Threw expected error | Passed |
| Content attribute collision test | Throws `Submodule name collision` | Threw expected error | Passed |
| Standalone repo `flake-modules` check | `nix flake check` on standalone library | All checks passed | Passed |
| `middle-earth` integration with `flake-modules` | `nix flake check` via locked GitHub input | All checks passed | Passed |
| `nixosConfigurations.gandalf` evaluation | Evaluates toplevel derivation via `flake-modules` | Clean evaluation | Passed |

## Session: 2026-09-24 (Opinionated Entrypoint & Directory Contract)

### Current Status
- **Phase:** 8 - Opinionated Entrypoint (`flake-modules.nix`) & Directory Contract
- **Status:** Complete
- **Active Plan:** `.planning/2026-09-23-declarative-flake-module-evaluator/`

### Actions Taken
- Refactored `evalFlake` in `flake-modules/lib/default.nix` to strictly receive a root directory argument (`rootDir`, e.g. `./.`), enforcing that `flake-modules.nix` must reside in that directory.
- Added strict error handling:
  - If `flake-modules.nix` is missing in `rootDir`, throws `flake-modules: Directory '<dir>' does not contain 'flake-modules.nix'.`
  - If a file path is passed instead of a directory, throws `flake-modules: evalFlake expects a directory path containing 'flake-modules.nix' (e.g. evalFlake ./. inputs), but received file path '<path>'.`
- Replaced inline `root = { mod, pubMod, createFlakeModule }: ...` with homogeneous root evaluation of `flake-modules.nix` receiving `{ createFlakeModule, pubMod, mod, declareNixosModule, self, super, flake, ... }`.
- Supported both curried invocation (`evalFlake ./. inputs`) and attribute-set invocation (`evalFlake { inherit inputs; rootDir = ./.; }`).
- Updated `template/flake-modules.nix` and `template/flake.nix` in `flake-modules` to use `evalFlake ./. inputs;`.
- Updated `README.md` in `flake-modules` with the directory-based invocation.
- Committed and pushed commits `d5c2aac` and `14961c5` to `main` on GitHub (`tarberd/flake-modules`).
- Created `/home/tarberd/middle-earth/flake-modules.nix` with all root submodules and public outputs.
- Reduced `/home/tarberd/middle-earth/flake.nix` outputs to a single line: `outputs = inputs@{ flake-modules, ... }: flake-modules.lib.evalFlake ./. inputs;`.
- Updated `flake.lock` in `middle-earth` via `nix flake update flake-modules` to commit `14961c5`.
- Staged `flake-modules.nix` in git.
- Verified end-to-end evaluation with `nix flake check`, `nix flake show`, and NixOS toplevel derivation evaluation for `gandalf`.

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| `flake-modules` clean check | `nix flake check` with directory contract logic | All checks passed | Passed |
| Missing `flake-modules.nix` error | Throws `Directory '<dir>' does not contain 'flake-modules.nix'` | Threw expected error | Passed |
| File path rejected error | Throws `evalFlake expects a directory path` | Threw expected error | Passed |
| Directory root path invocation | `fm.evalFlake ./template { ... }` correctly resolves `flake-modules.nix` | Evaluated `{ packages = ...; }` | Passed |
| `middle-earth` flake check | `nix flake check` via `evalFlake ./. inputs` | All checks passed | Passed |
| `middle-earth` flake show | Shows only `apps`, `packages`, `nixosConfigurations` | Clean public tree | Passed |
| `nixosConfigurations.gandalf` drvPath | Evaluates top-level NixOS system derivation | Evaluated successfully | Passed |

## Session: 2026-09-24 (Monadic Visibility Modifier: `pub mod`)

### Current Status
- **Phase:** 9 - Monadic Visibility Modifier (`pub mod`)
- **Status:** Complete
- **Active Plan:** `.planning/2026-09-23-declarative-flake-module-evaluator/`

### Actions Taken
- Implemented `pub` functor modifier in `flake-modules/lib/default.nix`, supporting both standalone/start-of-chain (`pub mod "foo"`) and chained (`mod "foo" pub mod "bar"`) syntax without parentheses.
- Added strict token validation: verifies `subOp.__type == "mod"` and throws a descriptive error if `mod` is omitted after `pub`.
- Completely removed `pubMod` without backward compatibility; only `mod` and `pub mod` are supported.
- Updated `moduleArgs` in both `evalModule` and `evalFlake` to expose `pub` and `mod` (and removed `pubMod`).
- Updated `template/flake-modules.nix` and `README.md` in `flake-modules`.
- Verified `flake-modules` with `nix flake check` and negative tests.
- Committed and pushed commit `32d8b77` to GitHub `tarberd/flake-modules`.
- Updated `middle-earth/flake.lock` via `nix flake update flake-modules` to commit `32d8b77`.
- Migrated all 10 module declarations in `middle-earth` from `pubMod` to `pub mod`.
- Verified 0 remaining occurrences of `pubMod` across the codebase using `grep_search`.
- Verified `middle-earth` with `nix flake check`, `nix flake show`, building `packages.x86_64-linux.onehost`, and evaluating `nixosConfigurations.gandalf` and `sauron`.

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| `flake-modules` check | `nix flake check` passes with `pub mod` | All checks passed | Passed |
| `pubMod` removed from `lib` | `fm ? pubMod` is `false` | `false` | Passed |
| Missing `mod` after `pub` error | Throws `Expected 'mod' after 'pub'` | Threw expected error | Passed |
| Codebase `pubMod` grep | 0 matches across the repository | 0 matches | Passed |
| `middle-earth` check | `nix flake check` with `pub mod` | All checks passed | Passed |
| `middle-earth` show | Shows only `apps`, `packages`, `nixosConfigurations` | Clean public tree | Passed |
| `nixosConfigurations.gandalf` drvPath | Evaluates top-level NixOS system derivation | Evaluated successfully | Passed |
| `nixosConfigurations.sauron` drvPath | Evaluates top-level NixOS system derivation | Evaluated successfully | Passed |

## Session: 2026-09-24 (Canonical NixOS Module Protocol & Reserved Keyword Hardening)

### Current Status
- **Phase:** 10 - Canonical NixOS Module Protocol & Reserved Keyword Hardening
- **Status:** Complete
- **Active Plan:** `.planning/2026-09-23-declarative-flake-module-evaluator/`

### Actions Taken
- Analyzed Nixpkgs `lib/modules.nix` and `lib/types.nix` module loading pipeline (`loadModule`, `unifyModuleSyntax`, `collectStructuredModules`, `isDisabled`).
- Confirmed Nixpkgs does not provide a built-in deduplicating wrapper (`setDefaultModuleLocation` only sets `_file` and `imports = [ m ]`, omitting `key` and failing deduplication).
- Replaced custom sha256 hash in `declareNixosModuleFor` with canonical string file path `toString filePath` for `key` and `_file`, restoring 100% compatibility with NixOS `disabledModules` path matching.
- Added support for author-defined `key` and `_file` declarations inside modules.
- Lifted attribute set modules to functors in `declareNixosModuleFor` so that attached submodules reside exclusively on the outer functor attribute set and do not leak into NixOS `unifyModuleSyntax` `attrsToRemove` checks.
- Preserved all NixOS reserved keywords (`_file`, `key`, `_class`, `disabledModules`, `imports`, `options`, `config`, `meta`, `freeformType`).
- Ran extensive positive and negative test suite evaluating deduplication, path string disabling, direct module reference disabling, author-defined key overriding, and submodule access.
- Committed and pushed commit `19993ad` to `main` on GitHub (`tarberd/flake-modules`).
- Updated `middle-earth/flake.lock` via `nix flake update flake-modules` to commit `19993ad`.
- Verified `middle-earth` with `nix flake check`, `nix flake show`, building `packages.x86_64-linux.onehost`, and evaluating `nixosConfigurations.gandalf` and `sauron`.

### Test Results
| Test | Expected | Actual | Status |
|------|----------|--------|--------|
| Multiple function module imports | Deduplicates cleanly without collision | Successfully deduplicated | Passed |
| Multiple attrset module imports | Deduplicates cleanly without collision | Successfully deduplicated | Passed |
| Submodule on attrset module | Submodules do not leak into NixOS module syntax | Clean eval, no unsupported attribute | Passed |
| `disabledModules` by string path | Disables module matching string path | Module disabled cleanly | Passed |
| `disabledModules` by direct reference | Disables module matching module attrset | Module disabled cleanly | Passed |
| Author-defined custom `key` | Respects custom key and disables cleanly | Custom key preserved and disabled | Passed |
| Invalid `declareNixosModule` argument | Throws descriptive type error | Threw expected error | Passed |
| `flake-modules` flake check | `nix flake check` passes | All checks passed | Passed |
| `middle-earth` flake check | `nix flake check` passes | All checks passed | Passed |
| `middle-earth` flake show | Shows only `apps`, `packages`, `nixosConfigurations` | Clean public tree | Passed |
| `onehost` package build | Rust package builds cleanly | Successfully built | Passed |
| `nixosConfigurations.gandalf` drvPath | Evaluates top-level NixOS system derivation | Evaluated successfully | Passed |
| `nixosConfigurations.sauron` drvPath | Evaluates top-level NixOS system derivation | Evaluated successfully | Passed |

## Session: 2026-09-24 (TDD & CI Test Infrastructure with nix-unit)

### Current Status
- **Phase:** 11 - TDD & CI Test Infrastructure (`tests/` Subflake & `nix-unit`)
- **Status:** Complete
- **Active Plan:** `.planning/2026-09-23-declarative-flake-module-evaluator/`

### Actions Taken
- Created isolated `tests/` subflake in `flake-modules/tests/flake.nix` consuming parent library via `path:..` and locking `nixpkgs` + `nix-unit`.
- Preserved the zero-dependency invariant (`inputs = {};`) on root `flake-modules/flake.nix`.
- Implemented minimal comprehensive unit test suite in `flake-modules/tests/test_lib.nix` (34 tests) covering:
  - Combinator syntax and left-to-right chaining (`pub mod`, `mod`, `createFlakeModule`).
  - Strict duplicate submodule detection (`mod`/`mod`, `pub`/`pub`, `pub`/`mod`).
  - Early type validation and missing `mod` after `pub` error matching.
  - Submodule resolution for single files, directories, missing paths, and ambiguous collisions.
  - Content merging, function functor lifting, and attribute key collisions.
  - NixOS module protocol, canonical `key`/`_file` stamping, author overrides, and invalid type errors.
  - NixOS module deduplication, disabling by path string, disabling by direct module reference, and submodule isolation.
  - Root `evalFlake` contract: missing entrypoint, file path rejection, privacy enforcement, lexical scopes (`self`, `super`, `flake`), and curried/attrset invocations.
  - Starter template evaluation.
- Added test fixtures in `tests/fixtures/` (`empty-dir`, `resolution/`, `sample-flake/`).
- Wrapped `nix-unit` execution inside `checks.<system>.unit-tests` so standard `nix flake check ./tests` executes the test suite in a hermetic build sandbox.
- Added GitHub Actions CI workflow in `.github/workflows/ci.yml` verifying root zero-dependency schema and running `nix flake check ./tests`.
- Verified test suite passes locally with `nix flake check ./tests` (34/34 tests passed).
- Committed and pushed commit `3ef3df3` to `main` on GitHub (`tarberd/flake-modules`).
- Verified and iterated on GitHub Actions remote execution:
  - Fixed action repository casing (`DeterminateSystems/nix-installer-action` and `DeterminateSystems/magic-nix-cache-action`).
  - Added required workflow permissions (`id-token: write`, `contents: read`).
  - Disabled FlakeHub login requirement (`use-flakehub: false`) for `magic-nix-cache-action` to eliminate warning annotations.
  - Pushed commits `3e576ba` and `e47dc24` to `main`.
  - Addressed all 4 GitHub Actions warning annotations and notices:
    - Upgraded `actions/checkout@v4` to `@v6` to run on Node.js 24 and eliminate the Node 20 deprecation warning.
    - Set `determinate: false` on `DeterminateSystems/nix-installer-action@main` and removed unnecessary `id-token: write` permission to prevent `determinate-nixd` from attempting FlakeHub login on unregistered repositories.
    - Pinned runner to `ubuntu-24.04` to eliminate the `ubuntu-latest` OS migration notice.
  - Pushed commit `5df7da1` and monitored workflow run `35997909719`:
    - Zero annotations, zero warnings, zero deprecation notices.
    - `Zero-Dependency Check` passed in 32s.
    - `nix-unit Test Suite & NixOS Matrix` passed in 1m 0s.
  - Implemented Option A (Major Version Tag Pinning):
    - Replaced moving `@main` references with semver major tags `DeterminateSystems/nix-installer-action@v23` and `DeterminateSystems/magic-nix-cache-action@v15`.
    - Pushed commit `b52fe30` and monitored workflow run `35999015644`.
    - Both jobs passed cleanly with 0 errors and 0 warnings (`Zero-Dependency Check` in 31s, `nix-unit Test Suite & NixOS Matrix` in 51s).

### Test Results
| Test Suite / Check | Total Tests | Passed | Warnings | Status |
|--------------------|-------------|--------|----------|--------|
| `nix-unit` Combinators & Syntax | 12 | 12 | 0 | Passed |
| `nix-unit` Submodule Resolution | 5 | 5 | 0 | Passed |
| `nix-unit` Content Merging & Collisions | 4 | 4 | 0 | Passed |
| `nix-unit` NixOS Protocol & Deduplication | 8 | 8 | 0 | Passed |
| `nix-unit` Root Evaluator Contract | 4 | 4 | 0 | Passed |
| `nix-unit` Starter Template Evaluation | 1 | 1 | 0 | Passed |
| Total `test_lib.nix` assertions | **34** | **34** | **0** | **Passed** |
| Root Flake Check (`nix flake check`) | N/A | Schema valid | 0 | Passed |
| Subflake CI Check (`nix flake check ./tests`) | N/A | Derivation built | 0 | Passed |
| GitHub Actions `check-root-flake` | N/A | Zero-dep check | 0 | Passed (31s) |
| GitHub Actions `run-unit-tests` | N/A | 34 tests in sandbox | 0 | Passed (51s) |
| Action Pinning (`@v23`, `@v15`, `@v6`) | N/A | Pinned semver tags | 0 | **Verified** |
| Total GitHub Actions Annotations | N/A | 0 | 0 | **Clean (0 warnings)** |


