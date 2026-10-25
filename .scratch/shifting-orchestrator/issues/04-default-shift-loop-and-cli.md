# Default Shift Loop Convention and CLI Binary

Status: ready-for-agent
Blocked by: 02, 03

## Description

Implement the default batteries-included loop convention and CLI binary:
- Conventional directory `.shifting/` containing:
  - `plan.sh`
  - `work.sh`
  - `verify.sh`
  - `commit.sh`
  - `rollback.sh` (minimal non-destructive stub: `exit 0`)
- `DefaultShiftLoop`: Pre-wires the 5 scripts into the canonical pipeline:
  - Wires `plan.sh` via categorical branching: exit code `0` maps to `Either::Left(stdout)` (proceeding to `work.sh`), exit code `2` maps to `Either::Right(LoopFlow::Break(()))` (clean loop termination), other codes error.
  - Wires `work.sh` as a `ScriptStep`.
  - Wires `verify.sh` as a `ScriptBranch`: exit code `0` passes to `commit.sh`, non-zero passes to `rollback.sh`.
  - Maps successful commit or rollback to `LoopFlow::Continue(Idle)` for next iteration.
- Standalone CLI binary `shift` (in `packages/shifting/src/main.rs`) using `clap` (derive):
  - Flags: `--dir <path>` (defaults to `.shifting`), `--max-iterations <N>`, `--verbose`.
  - Discovers scripts, constructs `DefaultShiftLoop`, runs via `Executor`, and reports summary.

## Verification Seam

**`Loop & CLI` Seam**: End-to-end 5-script composition, exit 2 break routing, CLI arguments via `clap`, directory discovery.
- Catches: Script pipeline mismatches, broken argument handling, non-terminating loops.
- Misses: Nix hermetic build packaging.

## Acceptance Criteria

- [ ] Works out of the box with any repository providing `.shifting/` scripts.
- [ ] `rollback.sh` is a minimal non-destructive stub (`exit 0`).
- [ ] Categorical branching routes `plan.sh` exit 2 directly to `LoopFlow::Break(())`.
- [ ] Standalone binary `shift` uses `clap` (derive) supporting `--dir`, `--max-iterations`, and `--verbose`.
- [ ] Integration test simulating full multi-cycle execution with mock scripts verifying planning, execution, verification, commit, rollback, and exit 2 clean break.
- [ ] Passes `cargo test` and `cargo clippy --all-targets -- -D warnings`.
