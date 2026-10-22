# 02: Physical Disk State Verification in Lifecycle and Disaster Recovery Tests

**What to build:** Updates lifecycle, pipeline, and disaster recovery tests to verify storage outcomes by inspecting physical disk images and files on the filesystem rather than tracking recorded storage action call logs. Tests verify that copy-on-write overlays are physically created, backing file references resolve to expected base images, and destroyed overlays are unlinked.

**Blocked by:** 01: Storage Unit Tests on Real Toolchain in Temporary Directories

**Status:** resolved

- [x] Domain lifecycle applier and destroyer tests assert on physical overlay presence, backing chain integrity, and disk unlinking in temporary directories.
- [x] Disaster recovery and pipeline integration tests eliminate assertions on recorded storage actions.
- [x] No lifecycle or integration test asserts on internal storage method invocation logs.
