# 03: Repository preconditions

Status: ready-for-agent
Blocked by: 02

## What to build

shift refuses to start anywhere a rollback could destroy the operator's work or where an iteration would have no base commit to return to (spec section 2). Before any script runs, and before the subcommand is dispatched, so it covers every command, shift resolves the repository root through git and stops with a typed error if the root is outside a git work tree, if HEAD has no commit, or if the working tree has staged, unstaged or untracked changes. Ignored files don't count.

The checks run once per invocation. Between iterations the scripts keep the tree consistent, and the runtime doesn't re-check it. This is also what makes Ctrl+C safe in version 1: after an interrupted run leaves changes behind, the next run refuses to start until the operator cleans up.

## Acceptance criteria

- [ ] A root outside a git work tree is refused with a typed error, and no script runs.
- [ ] A repository whose HEAD has no commit is refused with a typed error, and no script runs.
- [ ] A working tree with staged changes, with unstaged changes, or with an untracked file is refused with a typed error, and no script runs.
- [ ] A working tree whose only extra files are ignored by git is accepted.
- [ ] Refusals exit non-zero and print the error on stderr.
- [ ] The checks run ahead of subcommand dispatch, so every existing subcommand is covered.
- [ ] `shift` CLI seam tests cover spec scenario 6's precondition cases, each asserting that the stub scripts' log stays empty.
- [ ] Tests and Clippy pass with warnings denied across all targets.
