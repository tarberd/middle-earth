# 02: Advisory Host File Locking for Mutating Commands

**What to build:** Prevents concurrent onehost processes from racing or corrupting storage overlays and libvirt configurations. All mutating commands (`apply`, `destroy`, `image build`) acquire an exclusive advisory file lock on `$XDG_STATE_HOME/onehost/onehost.lock` prior to performing any infrastructure checks or modifications. If another process holds the lock, onehost aborts immediately with a clear lock conflict error.

**Blocked by:** 01: Manifest Schema Simplification, Canonical Release Tuple, and Dynamic Content Hashing

**Status:** ready-for-agent

- [ ] Mutating commands (`apply`, `destroy`, `image build`) acquire an exclusive advisory file lock on `$XDG_STATE_HOME/onehost/onehost.lock` at command startup.
- [ ] Non-mutating commands (`plan`, `status`) do not acquire the exclusive lock.
- [ ] When the lock file is held by an active process, subsequent mutating invocations fail immediately with an informative lock contention error and non-zero exit code.
- [ ] The advisory lock is released reliably upon command completion or failure.
