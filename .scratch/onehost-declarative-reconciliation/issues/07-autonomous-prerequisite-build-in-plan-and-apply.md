# 07: Autonomous Prerequisite Image Build in Unified Plan and Apply Pipeline

**What to build:** Unifies base image provisioning and instance reconciliation into a single declarative command. `onehost plan` inspects required base images in the depot store and structures the plan into prerequisite image build actions followed by instance reconciliation actions, de-duplicating shared base images referenced by multiple instances. `onehost apply` automatically triggers prerequisite image builds (and missing ISO fetches) before provisioning instance copy-on-write overlays and defining domains in Libvirt.

**Blocked by:** 02: Advisory Host File Locking for Mutating Commands, 04: Stateless Orphan Domain Discovery and Purging, 05: Deterministic Extra ISO Mounting and Sanitized Guest Serial Streaming, 06: Internalized UUP ISO Fetcher Toolchain

**Status:** ready-for-agent

- [ ] `OnehostPlan` separates execution into a de-duplicated prerequisite image build phase followed by instance reconciliation actions.
- [ ] Shared base images referenced by multiple instances are scheduled for build only once in the plan.
- [ ] `onehost apply` executes required image builds prior to creating instance copy-on-write overlays.
- [ ] Overlay disks are provisioned backed by newly built or existing golden master base images with read-only permissions (`0444`).
- [ ] End-to-end reconciliation succeeds from a clean depot without requiring manual pre-build commands.
