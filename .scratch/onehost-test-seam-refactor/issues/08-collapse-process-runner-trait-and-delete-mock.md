# 08: Collapse Process Runner Trait and Delete Process Mock

**What to build:** Collapses the process runner trait into standard direct process execution in the image builder, removing generic execution parameters across image builder structs and CLI dispatchers. Deletes the process runner mock and recorded process actions from production sources, completing the mock elimination across the codebase.

**Blocked by:** 07: Image Builder Artifact State Verification

**Status:** resolved

- [x] Image builder uses direct system process execution, removing generic process runner parameters.
- [x] Process runner trait and in-memory mock process runner are removed from production sources.
- [x] Entire test suite passes under `nix develop .#onehost -c cargo test` with zero compiler or linter warnings.
