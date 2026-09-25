# Findings & Discoveries: Declarative Flake Module Evaluator

## 1. Problem Analysis & Root Cause

The current `flake.nix` evaluator relies on an automatic filesystem tree crawler:
- `readDirSafe` and `getNixTree` recursively inspect the filesystem starting at `./`.
- `evaluateAndFlatten` merges all sibling files/directories directly into `default.nix`:
  ```nix
  if evaluatedChildren ? default then
    recursiveUpdate evaluatedChildren.default (removeAttrs evaluatedChildren [ "default" ])
  else
    evaluatedChildren
  ```
- **Consequences**:
  1. `packages/onehost`: Because `packages/` contains `default.nix` and `onehost/`, `onehost` is auto-included and merged into `packages`. As a result, the flake output `packages` contains `packages.onehost`. Nix flake standard output schema expects `packages.<system>.<packageName>`. Nix flake check interprets `onehost` as an architecture string (like `x86_64-linux`) and fails with an error.
  2. Root directory crawling: Non-flake directories such as `.agent`, `dotfiles`, and `secrets` are traversed and merged into outputs as `unknown` flake outputs.
  3. Lack of scoping/privacy: Any helper file (like `firewalld/lib.nix` or `windows-versions.nix`) is auto-evaluated as a module and exported unless hidden.

## 2. Monadic Module Builder Architecture

Modules evaluate to a monadic structure:
```nix
{
  __type = "flakeModule";
  privateModules = [ ... ];
  publicModules = [ ... ];
  content = ...;
}
```

### The Three Monadic Combinator Functions
1. `createFlakeModule`: Base constructor wrapping the module's target value / content.
   - Accepts attribute sets: `createFlakeModule { attribute = "value"; }`
   - Accepts empty attribute set: `createFlakeModule {}`
   - **Accepts function values**: `createFlakeModule ({}: {})` or `createFlakeModule ({ lib, config, ... }: { ... })`. When a function value is passed, the evaluator turns it into a callable functor attrset with `__functionArgs` and `_file` metadata, allowing submodules to be attached while remaining callable as a function.
2. `pubMod`: Functor combinator that registers a public submodule name (`publicModules`).
3. `mod`: Functor combinator that registers a private submodule name (`privateModules`).

### Combinator Chaining Syntax
Because `pubMod`, `mod`, and `createFlakeModule` are callable attribute sets (with `__functor` and `__type` metadata), left-associative Nix function application allows natural unparenthesized chaining:
```nix
pubMod "alice"
pubMod "bob"
mod "charlie"
createFlakeModule {
  attribute = "value";
}
```
This evaluates directly to:
```nix
{
  __type = "flakeModule";
  privateModules = [ "charlie" ];
  publicModules = [ "alice" "bob" ];
  content = { attribute = "value"; };
}
```

## 3. Scopes: `self`, `super`, and `flake` (Strictly No Aliases)

Every module receives strictly:
1. `self`:
   Contains `content // allSubmodules` (both private and public). The module accesses its own private submodules via `self.<submodule>`.
2. `super`:
   Points to the parent module's `self` scope. Child submodules have full access to parent items and private sibling submodules, mirroring Rust's `super::`.
3. `flake`:
   Points to the root flake namespace. Any module anywhere in the hierarchy can access top-level project namespaces (e.g. `flake.middle-earth.users` or `flake.packages.x86_64-linux.onehost`), mirroring Rust's `crate::`.

All old references to `selfModule` and `superModule` across the codebase are to be eliminated and updated to `self` and `super`.

## 4. Evaluator Scoping & Export Rules
- **Public Export**: When an external module or parent accesses module `M`, it receives:
  `content // evaluatedPublicModules`
  Private submodules (`privateModules`) are strictly excluded from the exported value.
- **Function Values**: If `content` is a function, it is wrapped in a functor so submodules can be attached as attributes while the module remains callable.
- **Re-exports**: A module can re-export private submodules or items under any name or path in its `content`:
  ```nix
  mod "onehost"
  createFlakeModule {
    x86_64-linux.onehost = self.onehost; # re-exported under architecture
  }
  ```
- **Flake Root**: `flake.nix` itself uses the monadic builder to declare private root namespaces and public flake outputs:
  ```nix
  mod "middle-earth"
  mod "gameservers"
  mod "firewalld"
  mod "dotman2nix"
  pubMod "apps"
  pubMod "packages"
  pubMod "nixosConfigurations"
  createFlakeModule {}
  ```

## 5. Evaluator Deep Revision Discoveries

1. **Dead Code in `evalModule`**:
   `actualPath` tests `builtins.pathExists (modulePath + "/default.nix")`. Because `resolveSubmodule` already resolves either `foo.nix` or `foo/default.nix`, `modulePath` is guaranteed to be a `.nix` file. Testing for `default.nix` on it is dead code.
2. **Dead Path Handling in `resolveSubmodule`**:
   `if builtins.isPath nameOrPath` is non-functional because `builtins.listToAttrs` strictly requires string attribute names. Submodule declarations must strictly be string names.
3. **Redundant Terminal Step**:
   `createFlakeModule` functor duplicates the `{ __type = "flakeModule"; ... }` attribute set creation instead of delegating to `builder [] [] createFlakeModule`.
4. **Silent Privacy Overwrite**:
   Declaring a module in both `mod` and `pubMod` silently defaulted to public because `allSubmodules = evaluatedPrivate // evaluatedPublic`. Must throw a collision error.
5. **Silent Submodule/Content Overwrite**:
   In `base // subs`, if `base` (module content) defines an attribute key matching a submodule name, `subs` silently overwrites it. Must detect collisions.
6. **Inconsistent Top-Level Outputs in `evalFlake`**:
   `evalFlake` used an ad-hoc `if builtins.isAttrs rootMonad.content then ... else evaluatedPublicRoot`, silently dropping non-attribute set content, whereas child modules used `mergeSubmodules`. Unifying on `mergeSubmodules` ensures identical behavior.
7. **Type Validation at Declaration**:
   `pubMod` and `mod` should validate that `name` is a string immediately to fail early with precise context.
8. **Deduplication of Mapping Logic**:
   `normalizeModuleList` and `normalizeRootList` duplicate identical mapping logic and can be unified into `evalSubmodules = evalFn: list: ...`.

## 6. Standalone Flake Extraction (`flake-modules`)

1. **Zero External Dependencies**:
   The entire evaluator is written using pure Nix language primitives (`builtins`, list/attr operations, hashes). No dependency on `nixpkgs`, `flake-utils`, or any third-party library is required. The library flake itself has `inputs = {};`.
2. **Library Interface**:
   The standalone library exports:
   - `lib.evalFlake`: The main evaluation engine.
   - `lib.mkFlake`: Convenient alias to `evalFlake`.
   - `lib.createFlakeModule`, `lib.pubMod`, `lib.mod`: Monadic combinators.
   - `lib.declareNixosModuleFor`: NixOS module key-stamping helper.
   - `templates.default`: Starter template for new consumers.
3. **Rust-Style Encapsulation vs Flake-Parts**:
   Unlike `flake-parts` which flattens options into a global schema, `flake-modules` guarantees:
   - Private submodules (`mod`) never leak into flake outputs or external consumers.
   - Predictable lexical navigation via `self`, `super`, and `flake`.
   - Complete isolation between parallel namespaces in a monorepo.

## 7. Opinionated Entrypoint (`flake-modules.nix`)

1. **Syntax Consistency**:
   Previously, root modules were declared via an inline `root = { mod, pubMod, createFlakeModule }: ...` callback in `flake.nix`. This was inconsistent with every other module in the repository, which uses the standard `{ createFlakeModule, pubMod, mod, ... }:` file signature.
2. **Dedicated Entrypoint File**:
   Standardizing on `flake-modules.nix` in the flake root ensures that the root module is a 100% first-class module evaluated identically to child modules.
3. **Directory Contract & Missing Entrypoint Error**:
   Instead of consumers specifying the file name, `evalFlake` strictly receives the root directory (`rootDir`, e.g. `./.`). By opinionated convention, `evalFlake` looks for `rootDir + "/flake-modules.nix"`. If `flake-modules.nix` is missing from that directory, it throws: `flake-modules: Directory '...' does not contain 'flake-modules.nix'.` If a file path is accidentally passed, it throws an informative error explaining that `evalFlake` expects a directory path containing `flake-modules.nix`.
4. **Hermetic Root Scoping**:
   Inside `flake-modules.nix`:
   - `self` contains root content and all submodules (public and private).
   - `super` is `null` (since there is no parent above the root).
   - `flake` points to `selfRef` (the entire flake namespace).
   - `publicOutputs` exports only modules declared with `pubMod` plus any content in `createFlakeModule`, preserving total privacy for internal modules declared with `mod`.

## 8. Monadic Visibility Modifier (`pub mod`)

1. **Syntactic Left-Associativity in Nix**:
   In Nix, `pub mod "foo"` parses as `((pub mod) "foo")`. In a chained sequence of module declarations (`mod "a" pub mod "b" createFlakeModule {}`), Nix evaluates `((((((mod "a") pub) mod) "b") createFlakeModule) {})`.
2. **Two-Stage Functor Application**:
   - **Standalone / Chain Start**: `pub` is an attribute set with `__functor` accepting `mod`, validating that `subOp.__type == "mod"` and returning `name: builder [] [ name ]`.
   - **In-Chain**: When an existing `builder priv pub` receives `pub`, it transitions into an intermediate modifier state expecting `mod`. When `mod` is passed, it returns a function that appends the subsequent module name to `pubList`.
3. **Rust Parity**:
   Replaces `pubMod "name"` with `pub mod "name"`, achieving a 1-to-1 syntactic mirror of Rust's `mod foo;` and `pub mod foo;` with zero parentheses.
4. **Zero Backward Compatibility**:
   Because `flake-modules` is in early development, `pubMod` is completely removed rather than aliased. Only `pub mod` is supported, keeping the codebase lean, idiomatic, and unambiguous.

## 9. Canonical NixOS Module Protocol & Reserved Keyword Analysis

1. **Upstream Nixpkgs Module Loading Mechanics (`nixpkgs/lib/modules.nix`)**:
   - Nixpkgs does not provide an off-the-shelf deduplicating module wrapper.
   - `lib.setDefaultModuleLocation file m` only injects `{ _file = file; imports = [ m ]; }` without setting `key`. It explicitly documents: *"This function does not add support for deduplication and disabledModules, although that could be achieved by wrapping the returned module and setting the key module attribute."*
   - When NixOS evaluates an in-memory module (function or attrset) lacking `key`, `loadModule` assigns an anonymous key (`${parentKey}:anon-${toString n}`). Because each import site generates a distinct `:anon-*` key, importing an in-memory module multiple times bypasses NixOS deduplication and causes duplicate option definition errors.
2. **Canonical `key` Protocol (`key = toString filePath;`)**:
   - Nixpkgs uses string file paths (`toString filePath`) as module keys, not sha256 hashes.
   - NixOS `disabledModules` checks `elem structuredModule.key disabledKeys`. When an entry is disabled using a path string (e.g. `"/path/to/mod.nix"`) or path literal (`./mod.nix`), NixOS compares against `structuredModule.key`.
   - A sha256 hash completely broke `disabledModules` path matching. Restoring `key = toString filePath;` aligns 100% with upstream Nixpkgs conventions and enables disabling modules via path string, path literal, or direct module reference (`flake.someModule`).
3. **Submodule Attribute Leakage into NixOS Module Syntax (`attrsToRemove`)**:
   - Nixpkgs `unifyModuleSyntax` checks `removeAttrs m attrsToRemove == {}` when `options` or `config` is present:
     `attrsToRemove = [ "_class" "_file" "key" "disabledModules" "imports" "options" "config" "meta" "freeformType" ];`
   - If an attribute set module has attached submodules (`base // subs`), NixOS sees each submodule as an unsupported attribute and throws:
     `error: Module '...' has an unsupported attribute '<submodule>'. This is caused by introducing a top-level config or options attribute.`
   - In shorthand syntax (without `options` or `config`), unrecognized attributes are placed directly into `config`, which causes `The option '<submodule>' does not exist.`
4. **Functor Lifting Solution**:
   - By lifting both module functions AND attribute sets into callable functors:
     ```nix
     {
       __functor = _functorSelf: moduleArgs: ...;
       __functionArgs = ...;
       key = modKey;
       _file = modFile;
     }
     ```
   - NixOS identifies the module as callable via `lib.isFunction` (`m ? __functor`).
   - NixOS invokes the functor, which returns ONLY the clean module attribute set (`options`, `config`, `imports`, `_class`, `meta`, `freeformType`, `disabledModules`, `key`, `_file`).
   - Any submodules attached via `mergeSubmodules` reside strictly on the outer functor set (`_functorSelf`). They are accessible via lexical attribute indexing (`flake.middle-earth.users.root`) but never returned during NixOS module evaluation, eliminating attribute leakage.
5. **Reserved Keyword Preservation**:
   - All 9 reserved keywords (`_file`, `key`, `_class`, `disabledModules`, `imports`, `options`, `config`, `meta`, `freeformType`) are preserved intact when returned by the module definition.
   - If a module author explicitly defines `key` or `_file`, `res.key or modKey` respects the author's explicit declaration.

## 10. TDD and CI Test Infrastructure (`tests/` Subflake & `nix-unit`)

1. **The Subflake Sandbox Pattern**:
   - For pure library flakes requiring zero public dependencies (`inputs = {}`), putting test dependencies (`nixpkgs`, test runners, linters) in the root `flake.nix` is an anti-pattern because it pollutes all downstream consumers' lockfiles with transitive dependencies.
   - The established industry pattern (used by `flake-parts`, `haumea`, `devshell`) isolates test dependencies into a nested `tests/` subflake consuming the parent library via `inputs.flake-modules.url = "path:..";`.
2. **`nix-unit` for Exact Assertion Matching**:
   - Unlike pure Nix `builtins.tryEval` (which only returns `{ success = false; }` without exposing the thrown error message string), `nix-unit` hooks directly into the Nix C++ evaluation engine.
   - It supports `expectedError = { type = "ThrownError"; msg = "..."; };`, enabling exact verification of descriptive error strings for duplicate modules, type errors, ambiguous paths, and missing entrypoints.
3. **Dual Execution Architecture**:
   - **Local Fast TDD**: `nix-unit` evaluates in-memory in ~30ms with colored diffs and line number traces.
   - **Universal CI (`nix flake check`)**: Packaging `nix-unit` inside a `checks` derivation in `tests/flake.nix` allows standard `nix flake check ./tests` to execute without requiring manual installation of `nix-unit` on CI runners or developer machines.

## 11. GitHub Actions CI Hardening & Warning Elimination

1. **Node.js 20 Deprecation (`actions/checkout@v6`)**:
   - GitHub Actions announced the removal of Node 20 from hosted runners. Actions targeting Node 20 (such as `actions/checkout@v4`) trigger runner deprecation warnings.
   - Updating to `actions/checkout@v6` runs on Node 24 and eliminates all runner deprecation annotations.
2. **FlakeHub Login Failure (`determinate: false`)**:
   - `DeterminateSystems/nix-installer-action` defaults to `determinate: true`, which installs the Determinate Nix daemon (`determinate-nixd`) and attempts to authenticate against FlakeHub using GitHub Actions OIDC tokens whenever `id-token: write` is present.
   - For repositories not registered on FlakeHub, `determinate-nixd` logs a non-fatal warning annotation: `FlakeHub Login failure: The process '/usr/local/bin/determinate-nixd' failed with exit code 1`.
   - Setting `determinate: false` installs standard upstream Nix with multi-user daemon and KVM sandboxing without launching `determinate-nixd` or requesting FlakeHub authentication.
   - Removing unnecessary `id-token: "write"` permissions adheres to the principle of least privilege (`contents: "read"` only).
3. **Runner Stability (`ubuntu-24.04`)**:
   - Specifying `ubuntu-24.04` rather than `ubuntu-latest` provides deterministic LTS runner environments and avoids the upcoming `ubuntu-latest` migration notices.
