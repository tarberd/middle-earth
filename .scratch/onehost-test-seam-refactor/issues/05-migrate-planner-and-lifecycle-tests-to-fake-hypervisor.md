# 05: Migrate Planner and Lifecycle Tests to Fake Hypervisor State Assertions

**What to build:** Migrates domain lifecycle, planner, and end-to-end integration tests to use the shared fake hypervisor fixture. Replaces all assertions on recorded hypervisor action call logs with queries against observable domain state (verifying domain registration, state transitions, XML definitions, and pool resolution).

**Blocked by:** 04: Shared State-Based Fake Hypervisor Test Fixture

**Status:** ready-for-agent

- [ ] Domain lifecycle applier and destroyer tests use the shared fake hypervisor and assert on observable domain state.
- [ ] Planner tests verify reconciliation actions using the shared fake hypervisor without interaction spies.
- [ ] Pipeline integration tests execute end-to-end lifecycle verification against observable hypervisor state.
