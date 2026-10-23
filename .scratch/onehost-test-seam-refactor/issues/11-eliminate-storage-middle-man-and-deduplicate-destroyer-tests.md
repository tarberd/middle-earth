# 11: Eliminate Storage Middle-Man Delegation and Deduplicate Destroyer Tests

**What to build:** Consolidates storage operations directly onto the concrete QEMU storage manager without middle-man delegation to duplicate public free functions, extracts shared backing-file argument construction across thin backup conversion and restoration, and deduplicates the graceful vs. forced domain destroyer tests.

**Blocked by:** 10: Expression-Oriented Image Builder Validation and Backup Guard Combinators

**Status:** ready-for-agent

- [ ] `QemuImgStorage` directly owns safe file moves and content-hash computation without delegating to duplicate public free functions, and shares a single helper for backing-file CLI argument construction across `convert_thin_backup` and `restore_thin_backup`.
- [ ] Storage unit tests exercise `QemuImgStorage` methods directly with zero dead or duplicate free functions exported from the storage module.
- [ ] Graceful and forced domain destroyer tests share a common setup and verification helper instead of duplicating test bodies.
