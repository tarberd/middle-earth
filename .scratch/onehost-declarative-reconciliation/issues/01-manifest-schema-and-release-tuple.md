# 01: Manifest Schema Simplification, Canonical Release Tuple, and Dynamic Content Hashing

**What to build:** Manifest schema and CLI are updated to declare instances concisely and unambiguously. Each declared instance specifies a target storage pool and references a base image via a canonical Release Tuple (`uup_id`, `edition`, `language`, `flavor`), removing informal build tags and redundant OS namespaces. Manifest flavors declare an OEMDRV path and an arbitrary map of extra ISO paths without manual hash strings. Onehost dynamically calculates the Content Hash from the OEMDRV media and alphabetically sorted extra ISOs, resolving Base Image filenames deterministically. Legacy lifecycle policy blocks (`prevent_destroy`, `on_image_change`), `autostart`, and CLI override flags (`--allow-recreate`, `--allow-destroy-protected`) are removed.

**Blocked by:** None (can start immediately)

**Status:** resolved

- [x] Manifest deserialization accepts flavor declarations containing an `oemdrv_path` and `extra_isos: HashMap<String, PathBuf>`, rejecting manifests with legacy `hash` fields.
- [x] Manifest deserialization and CLI reject informal build strings and accept images specified as canonical Release Tuples (`<uup_id>/<edition>/<language>/<flavor>`).
- [x] Manifest requires explicit `pool` declarations on all instances and rejects legacy `default_pool`, `autostart`, and `lifecycle` blocks.
- [x] Onehost computes a streaming SHA-256 Content Hash across OEMDRV and alphabetically sorted extra ISOs, using the first 12 hexadecimal characters in the resolved Base Image filename.
- [x] CLI removes `--allow-recreate` and `--allow-destroy-protected` flags across all commands.

## Resolution
- Implemented `ImageTagSpecification` parsing and validating RFC-4122 canonical Release Tuples `<uup_id>/<edition>/<language>/<flavor>`, rejecting legacy informal tags (`win11/...`).
- Updated `ContentAddressedImageResolver` to format golden master filenames as `<uup_id>-<edition>-<language>-<flavor>-<content_hash>.qcow2` and raw depot ISOs as `<uup_id>-<edition>-<language>.iso`.
- Updated manifest model with `#[serde(deny_unknown_fields)]` across `OnehostManifest`, `FlavorConfiguration`, and `InstanceConfiguration`. Replaced legacy fields (`hash`, `default_pool`, `autostart`, `lifecycle`) with `extra_isos: HashMap<String, PathBuf>` and required `pool`.
- Implemented streaming SHA-256 content hashing in `StorageManager` (`execute_compute_content_hash`) over `oemdrv_path` followed by alphabetically sorted keys of `extra_isos` using `BTreeMap` and `io::copy`, truncating to 12 hex characters.
- Removed `--allow-recreate` and `--allow-destroy-protected` CLI options, flags, and guardrail logic from `DomainLifecyclePlanner`, `DomainLifecycleApplier`, and `DomainDestroyer`. Added tests asserting CLI parser rejection of these flags and rejection of informal build tags on `image build`.
- Updated all unit and integration test suites to pass under `nix develop .#onehost -c cargo test` and verified zero warnings under `cargo clippy --lib --bins`.
