# 10: Expression-Oriented Image Builder Validation and Backup Guard Combinators

**What to build:** Eliminates the remaining imperative early return, duplicate OEMDRV existence checks, and stale comments in the image builder and its tests, and replaces the side-effecting fold in the backup snapshot transaction guard with a value-producing iterator combinator.

**Blocked by:** 09: Pure State Snapshot and Running-State Lifecycle Transitions in FakeHypervisor

**Status:** resolved

- [x] Image builder validates OEMDRV existence once using an expression-oriented combinator with zero early `return` statements and no redundant `inspect_image` check on the OEMDRV ISO.
- [x] Stale `BuildScratchGuard` comments are removed from image builder tests.
- [x] Backup snapshot transaction guard cleanup uses a value-producing iterator combinator instead of `.fold((), ...)`.
