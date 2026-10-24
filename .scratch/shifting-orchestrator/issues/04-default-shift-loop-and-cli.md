# Default Shift Loop Convention and CLI Binary

Status: ready-for-agent
Blocked by: 03

## Description

Implement the default batteries-included loop convention and CLI binary:
- Conventional directory `.shifting/` containing:
  - `plan.sh`
  - `work.sh`
  - `verify.sh`
  - `commit.sh`
  - `rollback.sh`
- `DefaultShiftLoop`: Pre-wires the 5 scripts into the canonical pipeline: `Plan -> Work -> Verify Branch -> (Left: Commit | Right: Rollback) -> Continue/Break`.
- Provide a minimal standalone binary `shift` (in `packages/shifting/src/main.rs`) executing `DefaultShiftLoop::new().run()`.

## Acceptance Criteria

- [ ] Works out of the box with any repository providing `.shifting/` scripts.
- [ ] Verifies clean git working tree recovery when `verify.sh` exits non-zero and `rollback.sh` executes.
- [ ] Standalone binary runs cleanly and provides clear stdout/stderr logs.
- [ ] Passes `cargo test` and `cargo clippy --all-targets -- -D warnings`.
