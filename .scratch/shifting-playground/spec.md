# Shifting Playground: Sandboxed Nix Images for Ralph Loop Experiments

Status: ready-for-agent

## Problem Statement

The default loop runs its agent with permission prompts disabled (Farr's `--dangerously-skip-permissions`). An operator who runs it on a workstation exposes their SSH keys, git credentials, home directory and network to an agent that will do whatever its context suggests.

They also have no reproducible codebase to run it against, so they can't compare runs, can't tell whether a change to the prompts or scripts made the loop better, and can't try the loop on both a fresh and a populated codebase.

## Solution

The flake gains a playground: small container images built with Nix, plus a launcher.

- **Samples.** Each image carries one sample repository and its toolchain:
  - a hello-world Rust crate, for the fresh-codebase case;
  - the foot terminal emulator at a pinned release cloned from `https://codeberg.org/dnkl/foot`, for the populated-codebase case.
- **Working copies.** The launcher creates the operator's working copy of a sample on the host on first use. Each working copy already has `shift init` applied and its `project.sh` filled in. The launcher then runs `shift` inside the container.
- **Sandbox:**
  - the container receives only the model API key;
  - it mounts only the working copy, with `.shifting/` read-only;
  - it can reach only the model API, through an allowlisting proxy;
  - it has no remote to push to.
- **Report.** A report command summarises the run directories of a working copy, so that runs can be compared.
- **Checks.** Nix checks run the whole loop against hello-world with a fake agent, and confirm that foot's project checks pass at the pinned revision.

Topic specs are not shipped. The operator writes them with their own tools for each experiment.

## User Stories
1. As an operator, I want one `nix run` command that takes a sample name and a host working directory, so that starting an experiment is a single step.
2. As an operator, I want the launcher to create a working copy of the sample in my working directory the first time and reuse it afterwards, so that experiments persist and I can inspect or resume them.
3. As an operator, I want each new working copy to be its own git repository with a baseline commit, `shift init` applied and committed, and a repository-local git identity, so that the harness can commit from the first iteration.
4. As an operator, I want the hello-world working copy's `project.sh` filled in with `cargo clippy` (warnings denied), `cargo test` and a single-test runner, so that the fresh case works with no setup.
5. As an operator, I want the foot working copy pinned to release 1.28.0 from `https://codeberg.org/dnkl/foot`, with `project.sh` running foot's own meson build and tests and a single-test runner, so that the populated case is a real C codebase that always starts from the same state.
6. As an operator, I want the working copies to carry no topic specs, so that I write each experiment's specs with my own tools.
7. As an operator, I want any arguments after the sample and working directory passed straight to `shift`, so that I can run `shift plan`, `shift build`, `shift review` or a bounded run in the sandbox.
8. As an operator, I want an option that opens a shell in the container instead, so that I can inspect a working copy in the same environment the agent sees.
9. As an operator, I want the container to receive only the model API key from my environment, and no SSH agent, git credentials, home directory or other host mounts, so that the agent can't reach my secrets.
10. As an operator, I want `.shifting/` mounted read-only inside the container, so that the agent physically can't change the scripts that judge it, on top of the diff checks.
11. As an operator, I want the container's only network path to be an allowlisting proxy that permits the model API host, with the agent CLI's non-essential traffic turned off, so that the agent can't fetch or send anything else.
12. As an operator, I want working copies to have no git remote, so that nothing can be pushed from the sandbox.
13. As an operator, I want the container to run as my host user ID, not root, so that files in the working copy stay owned by me.
14. As an operator, I want a terminal passed through when I am attached, so that the interactive review stage works inside the container.
15. As an operator, I want each sample in its own image, with only that sample's toolchain on a shared base of `shift`, git, a shell, core utilities, jq, CA certificates and the agent CLI, so that the images stay small.
16. As an operator, I want each image to provide its sample's build environment the way a Nix dev shell would (compiler wrappers and pkg-config search paths), so that the project checks run without Nix inside the container.
17. As an operator, I want the agent CLI taken from nixpkgs and pinned by the flake lock, so that every run uses the same agent version until I update it.
18. As an operator, I want the images and launcher reproducible from the flake lock, so that two machines run the same experiment.
19. As an operator, I want a report command that reads a working copy's run directories and prints, per run:
    - iterations;
    - commits;
    - already-done tasks;
    - rollbacks by outcome;
    - declines;
    - assumptions filed;
    - judge verdicts;
    - total cost, taken from the transcripts,

    so that I can compare runs and changes to prompts or scripts.
20. As an operator, I want run directories kept in the working copy's git directory on the host, so that they survive the container and appear in the report.
21. As a developer, I want a Nix check that runs `shift` with a fake agent against a hello-world working copy, carrying one topic through planning, building, commit and `Done`, so that the playground's whole path is tested without model calls.
22. As a developer, I want a Nix check that runs foot's `project.sh` checks at the pinned revision, so that the populated sample is known to start green.
23. As a developer, I want the sample sources, fake-agent fixtures and filled `project.sh` files kept alongside the shifting package, so that the playground is versioned with the loop it exercises.

## Implementation Decisions

### 1. Images
- **Build.** Layered OCI images streamed from Nix (`dockerTools`). They share a base layer of:
  - `shift`, git, bash, coreutils, findutils, grep, sed, jq, CA certificates;
  - the nixpkgs `claude-code` package, which is unfree, so unfree use is allowed only for this package.
- **Hello-world image.** Adds a Rust toolchain: cargo, rustc, clippy, rustfmt.
- **Foot image.** Adds foot's build inputs as nixpkgs defines them for the foot package: meson, ninja, pkg-config, wayland-scanner, scdoc, ncurses, tllist, wayland, wayland-protocols, pixman, fontconfig, freetype, libxkbcommon, fcft, utf8proc, and a C compiler.
- **Build environment.** Each image exports the variables a dev shell for its sample would set, captured when the image is built.
- **User.** Each image defines an unprivileged user and a writable home directory.
- **Proxy.** A separate small image runs the allowlisting proxy.

### 2. Samples
- **hello-world.** A minimal Rust binary crate with one unit test, kept in this repository.
- **foot.** Fetched from `https://codeberg.org/dnkl/foot` at tag 1.28.0 (commit `ab33c9a19d626f8d0ac0bd1adfdba21946ada948`), with its content hash in the Nix expression.
- **Working copies.** Each starts from a source snapshot with no upstream history and no remote.
- **Filled `project.sh` per sample:**
  - hello-world: `project_checks` runs `cargo clippy --all-targets -- -D warnings` and `cargo test`; `run_named_test` runs `cargo test` for an exact test name.
  - foot: `project_checks` configures a meson build directory, builds it with ninja and runs `meson test`; `run_named_test` runs `meson test` for one test.
- Each sample ships a fake-agent fixture used only by the Nix checks.

### 3. Launcher
- **Invocation.** `nix run` of the playground app with `<sample> <workdir> [shift arguments…]`, or a shell option in place of the shift arguments.
- **First use** of a working directory, done inside the sample's image:
  1. copy the sample source;
  2. `git init`, set a repository-local identity, and commit the baseline;
  3. run `shift init`, install the sample's `project.sh`, and commit.
- **Container runtime.** Rootless podman on the host is required. Containers run:
  - with the working directory mounted read-write at the working path, and `.shifting/` mounted read-only over it;
  - as the host user's ID;
  - with a TTY when the launcher's stdin is a terminal;
  - with `ANTHROPIC_API_KEY` passed through and the agent CLI's non-essential traffic disabled;
  - on an internal network. Their only route out is the proxy container, which permits the model API host and nothing else. `HTTPS_PROXY` points at it.
- **Cleanup.** The launcher starts the proxy if it isn't running, and removes containers when they exit.

### 4. Report
- A script in the base image, also exposed as a launcher option.
- **Sources:**
  - the working copy's run directories;
  - the git log over each run's commits, for counting assumptions filed;
  - the build transcripts, for the final cost reported by the agent CLI.
- It reads the run summaries the runtime writes and the judge verdicts saved in each iteration directory.
- It prints one block per run and a total.

### 5. Checks
- Containers can't run inside the Nix build sandbox, so the checks run in an environment with the same contents as the hello-world or foot image, not in the image itself.
- **hello-world check:**
  1. create a working copy;
  2. add a fixture topic spec;
  3. point `SHIFT_AGENT`, `SHIFT_JUDGE_AGENT` and `SHIFT_REVIEW_AGENT` at the fixture fake agent;
  4. run `shift` with the interactive override `never`;
  5. assert the plan commit, the task commit, the task's `done` status and the `Done` end state.
- **foot baseline check.** Runs the filled `project_checks` on an untouched working copy.

## Testing Decisions

### What Makes a Good Test
Tests check what an operator would see in a working copy (commits, files, statuses, end state) and that the samples build. No test calls a real model. Real-model runs are experiments whose results go to the report, not to assertions.

### Seam: Nix checks over the playground environment
- **Location.** `nix flake check`, running the hello-world loop and foot baseline checks from section 5.
- **Catches:**
  - broken image contents or a broken build environment;
  - a `project.sh` that is filled in wrongly;
  - working-copy setup errors;
  - breakage when the default loop and the playground are combined.
- **Misses:**
  - container isolation (mounts, network, user), which is checked by hand on first launch against a short checklist in the launcher's help;
  - the behaviour of real agents.

### Prior Art
- The hermetic Nix devshell conventions in `docs/agents/validation.md`.
- The fake-agent CLI seam from `shifting-default-loop`, reused unchanged.

## Out of Scope
- Shipping topic specs or experiment scripts for the samples.
- Running the playground on macOS, or with Docker instead of podman.
- Any secret other than the model API key.
- Pushing results anywhere. Working copies stay local.
- Automatic comparison or grading of runs beyond the report.
- Tracking newer foot releases.

## Further Notes
- **Dependencies.** Depends on `shifting-loop-runtime`, `shifting-default-loop`, and on `shift` being a flake package (`shifting-orchestrator` issue 05).
- **Foot source.** Foot moved from sourcehut, whose newest tag is 1.13.1, to `https://codeberg.org/dnkl/foot`. 1.28.0 is the newest tag there at the time of writing.
  - It is also the version packaged in the flake's nixpkgs, so the foot image can take its build inputs from nixpkgs' foot package unchanged.
  - Moving to a newer release means updating the tag, commit and hash together, and rerunning the foot baseline check.
- **ADRs.** ADR-0011 (amended) and ADR-0012 apply to the loop being exercised.
- **Vocabulary.** Terms are defined in `GLOSSARY.md`.
