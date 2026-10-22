# 06: Internalized UUP ISO Fetcher Toolchain

**What to build:** Automates retrieval and preparation of official Windows installation ISOs when they are missing from the depot ISO cache. When a required Release Tuple ISO is not present at `<depot_iso_dir>/<uup_id>-<edition>-<language>.iso`, onehost invokes an internalized UUP fetcher toolchain (orchestrating `aria2`, `wimlib`, and `cabextract`) in an isolated temporary scratch directory, converting Microsoft update packages into a verified ISO and promoting it atomically to the depot ISO cache.

**Blocked by:** 01: Manifest Schema Simplification, Canonical Release Tuple, and Dynamic Content Hashing

**Status:** ready-for-agent

- [ ] Missing raw installation ISOs are detected at `<depot_iso_dir>/<uup_id>-<edition>-<language>.iso`.
- [ ] Toolchain downloads UUP dump packages into an isolated temporary scratch directory using `aria2`.
- [ ] Toolchain converts downloaded packages into a standard bootable ISO using `wimlib` and `cabextract`.
- [ ] Built ISO is atomically staged and promoted to `<depot_iso_dir>/<uup_id>-<edition>-<language>.iso`.
- [ ] Failures during download or conversion clean up scratch files and do not leave corrupted files in the depot ISO directory.
