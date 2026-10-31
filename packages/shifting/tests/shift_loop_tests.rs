use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use clap::Parser;
use shifting::cli::{self, Cli};

/// The seven loop scripts with the body each stub runs unless a test overrides it.
const STUBS: [(&str, &str); 7] = [
    ("select.sh", "exit 2"),
    ("precheck.sh", "exit 1"),
    ("work.sh", "echo \"summary of $SHIFT_TASK\""),
    ("verify.sh", "echo 'verify output'"),
    ("judge.sh", "echo \"verdict on $SHIFT_TASK\""),
    (
        "commit.sh",
        "git commit -q --allow-empty -m \"$SHIFT_OUTCOME $SHIFT_TASK\"",
    ),
    ("rollback.sh", "git reset -q --hard \"$SHIFT_BASE\""),
];

/// A temporary git repository with logging stub scripts.
///
/// The repository lives in `state/repository`. Each stub appends its name, environment,
/// working directory and stdin to `state/log`, then runs its body, which may keep counters in
/// `$state`.
struct Repository {
    state: tempfile::TempDir,
    root: PathBuf,
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git wrote invalid UTF-8")
        .trim_end()
        .to_string()
}

/// Writes an executable file through a child process, so that no handle to it stays open in
/// this multi-threaded test process while another test spawns a script.
fn write_executable(path: PathBuf, content: &str) -> PathBuf {
    let mut child = Command::new("sh")
        .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "sh"])
        .arg(&path)
        .stdin(Stdio::piped())
        .spawn()
        .expect("failed to spawn script writer");
    child
        .stdin
        .take()
        .expect("script writer has a stdin")
        .write_all(content.as_bytes())
        .expect("failed to write script");
    let status = child.wait().expect("failed to wait on script writer");
    assert!(status.success(), "script writer failed with {status}");
    path
}

/// A select body that selects `tasks` in order, each padded with whitespace, then reports no
/// work left. Further runs fail, so a loop that ignores the end can't run forever.
fn select_tasks(tasks: &[&str]) -> String {
    let arms = tasks
        .iter()
        .enumerate()
        .map(|(index, task)| format!("{}) printf '  %s \\n' '{task}' ;;\n", index + 1))
        .collect::<String>();
    format!(
        "n=$(($(cat \"$state/select.count\" 2>/dev/null || echo 0) + 1))\n\
         echo \"$n\" > \"$state/select.count\"\n\
         case \"$n\" in\n\
         {arms}\
         {}) exit 2 ;;\n\
         *) echo 'select ran after reporting no work left' >&2; exit 1 ;;\n\
         esac",
        tasks.len() + 1
    )
}

impl Repository {
    /// An empty repository; [`Repository::committed`] makes its first commit.
    fn new() -> Self {
        let state = tempfile::tempdir().expect("failed to create temp dir");
        let root = state.path().join("repository");
        std::fs::create_dir(&root).expect("failed to create repository");
        git(&root, &["init", "-q"]);
        git(&root, &["config", "user.name", "shift tests"]);
        git(&root, &["config", "user.email", "shift-tests@example.com"]);
        git(&root, &["config", "commit.gpgsign", "false"]);
        std::fs::write(state.path().join("log"), "").expect("failed to create log");
        let root = std::fs::canonicalize(&root).expect("failed to resolve repository");
        Self { state, root }
    }

    /// Adds `.shifting/<name>/` with every stub, `bodies` replacing the default bodies.
    fn with_loop(self, name: &str, bodies: &[(&str, &str)]) -> Self {
        self.with_loop_scripts(name, bodies, &STUBS.map(|(script, _)| script))
    }

    /// Adds `.shifting/<name>/` with only the stubs in `scripts`.
    fn with_loop_scripts(self, name: &str, bodies: &[(&str, &str)], scripts: &[&str]) -> Self {
        let dir = self.root.join(".shifting").join(name);
        std::fs::create_dir_all(&dir).expect("failed to create loop directory");
        let written = STUBS
            .iter()
            .filter(|(script, _)| scripts.contains(script))
            .map(|(script, default)| {
                let body = bodies
                    .iter()
                    .find(|(overridden, _)| overridden == script)
                    .map_or(*default, |(_, body)| body);
                write_executable(dir.join(script), &self.stub(script, body))
            })
            .collect::<Vec<_>>();
        assert_eq!(written.len(), scripts.len(), "unknown stub in {scripts:?}");
        self
    }

    fn stub(&self, script: &str, body: &str) -> String {
        format!(
            "#!/bin/sh\n\
             state='{}'\n\
             stdin=$(tr '\\n' '|')\n\
             printf '%s %s#%s task=%s base=%s outcome=%s cwd=%s stdin=%s\\n' \
             '{}' \"$SHIFT_LOOP\" \"$SHIFT_ITERATION\" \"${{SHIFT_TASK-<unset>}}\" \"$SHIFT_BASE\" \
             \"${{SHIFT_OUTCOME-<unset>}}\" \"$(pwd -P)\" \"$stdin\" >> \"$state/log\"\n\
             {body}\n",
            self.state.path().display(),
            script.trim_end_matches(".sh"),
        )
    }

    /// Commits everything as the commit `start`.
    fn committed(self) -> Self {
        git(&self.root, &["add", "-A"]);
        git(
            &self.root,
            &["commit", "-q", "--allow-empty", "-m", "start"],
        );
        self
    }

    /// Runs `shift` on this repository.
    fn shift(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_shift"))
            .arg("--root")
            .arg(&self.root)
            .args(args)
            .output()
            .expect("failed to run shift")
    }

    /// The stubs' log, with the repository root written `<root>` and each commit written
    /// `<subject>`.
    fn log(&self) -> String {
        let log =
            std::fs::read_to_string(self.state.path().join("log")).expect("failed to read log");
        git(&self.root, &["log", "--format=%H %s"])
            .lines()
            .filter_map(|line| line.split_once(' '))
            .fold(
                log.replace(&self.root.display().to_string(), "<root>"),
                |log, (hash, subject)| log.replace(hash, &format!("<{subject}>")),
            )
    }

    /// The subjects of the repository's commits, newest first.
    fn commits(&self) -> Vec<String> {
        git(&self.root, &["log", "--format=%s"])
            .lines()
            .map(str::to_string)
            .collect()
    }
}

/// `lines`, each ending in a newline.
fn lines(lines: &[&str]) -> String {
    lines.iter().map(|line| format!("{line}\n")).collect()
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("shift wrote invalid UTF-8 on stdout")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "shift failed with {}: {}",
        output.status,
        stderr(output)
    );
}

#[test]
fn test_build_loop_ends_exhausted_when_select_reports_no_work_left() {
    let repository = Repository::new().with_loop("build", &[]).committed();

    let output = repository.shift(&["build"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        "iterations: 0\n\
         commits: 0\n\
         already-done: 0\n\
         work-failed: 0\n\
         verify-failed: 0\n\
         judge-failed: 0\n\
         declines: 0\n\
         Exhausted\n"
    );
    assert_eq!(
        repository.log(),
        "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=\n"
    );
}

#[test]
fn test_build_loop_commits_verified_work_and_rolls_back_failed_checks() {
    let select = select_tasks(&["t1", "t2", "t3"]);
    let repository = Repository::new()
        .with_loop(
            "build",
            &[
                ("select.sh", &select),
                (
                    "verify.sh",
                    "echo 'verify output'\n\
                     [ \"$SHIFT_TASK\" != t2 ] || { echo 'tests failed for t2' >&2; exit 1; }",
                ),
                (
                    "judge.sh",
                    "[ \"$SHIFT_TASK\" != t3 ] || { echo 'judge rejects t3'; exit 1; }\n\
                     echo \"verdict on $SHIFT_TASK\"",
                ),
            ],
        )
        .committed();

    let output = repository.shift(&["build"]);

    assert_success(&output);
    assert_eq!(
        repository.log(),
        lines(&[
            "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "precheck build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "work build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "verify build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=summary of t1|",
            "judge build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=summary of t1|",
            "commit build#1 task=t1 base=<start> outcome=verified cwd=<root> stdin=verdict on t1|",
            "select build#2 task=<unset> base=<verified t1> outcome=<unset> cwd=<root> stdin=",
            "precheck build#2 task=t2 base=<verified t1> outcome=<unset> cwd=<root> stdin=t2|",
            "work build#2 task=t2 base=<verified t1> outcome=<unset> cwd=<root> stdin=t2|",
            "verify build#2 task=t2 base=<verified t1> outcome=<unset> cwd=<root> stdin=summary of t2|",
            "rollback build#2 task=t2 base=<verified t1> outcome=verify-failed cwd=<root> stdin=tests failed for t2|",
            "select build#3 task=<unset> base=<verified t1> outcome=<unset> cwd=<root> stdin=",
            "precheck build#3 task=t3 base=<verified t1> outcome=<unset> cwd=<root> stdin=t3|",
            "work build#3 task=t3 base=<verified t1> outcome=<unset> cwd=<root> stdin=t3|",
            "verify build#3 task=t3 base=<verified t1> outcome=<unset> cwd=<root> stdin=summary of t3|",
            "judge build#3 task=t3 base=<verified t1> outcome=<unset> cwd=<root> stdin=summary of t3|",
            "rollback build#3 task=t3 base=<verified t1> outcome=judge-failed cwd=<root> stdin=judge rejects t3|",
            "select build#4 task=<unset> base=<verified t1> outcome=<unset> cwd=<root> stdin=",
        ])
    );
    assert_eq!(
        stdout(&output),
        lines(&[
            "iterations: 3",
            "commits: 1",
            "already-done: 0",
            "work-failed: 0",
            "verify-failed: 1",
            "judge-failed: 1",
            "declines: 0",
            "Exhausted",
        ])
    );
    assert_eq!(repository.commits(), ["verified t1", "start"]);
}

#[test]
fn test_precheck_passing_commits_as_already_done_without_work() {
    let select = select_tasks(&["t1", "t2"]);
    let repository = Repository::new()
        .with_loop(
            "build",
            &[
                ("select.sh", &select),
                (
                    "precheck.sh",
                    "[ \"$SHIFT_TASK\" != t1 ] || { echo 't1 tests already pass'; exit 0; }\n\
                     kill -9 $$",
                ),
            ],
        )
        .committed();

    let output = repository.shift(&["build"]);

    assert_success(&output);
    assert_eq!(
        repository.log(),
        lines(&[
            "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "precheck build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "commit build#1 task=t1 base=<start> outcome=already-done cwd=<root> stdin=t1 tests already pass|",
            "select build#2 task=<unset> base=<already-done t1> outcome=<unset> cwd=<root> stdin=",
            "precheck build#2 task=t2 base=<already-done t1> outcome=<unset> cwd=<root> stdin=t2|",
            "work build#2 task=t2 base=<already-done t1> outcome=<unset> cwd=<root> stdin=t2|",
            "verify build#2 task=t2 base=<already-done t1> outcome=<unset> cwd=<root> stdin=summary of t2|",
            "judge build#2 task=t2 base=<already-done t1> outcome=<unset> cwd=<root> stdin=summary of t2|",
            "commit build#2 task=t2 base=<already-done t1> outcome=verified cwd=<root> stdin=verdict on t2|",
            "select build#3 task=<unset> base=<verified t2> outcome=<unset> cwd=<root> stdin=",
        ])
    );
    assert_eq!(
        stdout(&output),
        lines(&[
            "iterations: 2",
            "commits: 1",
            "already-done: 1",
            "work-failed: 0",
            "verify-failed: 0",
            "judge-failed: 0",
            "declines: 0",
            "Exhausted",
        ])
    );
}

#[test]
fn test_failed_or_killed_work_rolls_back_and_the_loop_continues() {
    let select = select_tasks(&["t1", "t2", "t3"]);
    let repository = Repository::new()
        .with_loop(
            "build",
            &[
                ("select.sh", &select),
                (
                    "work.sh",
                    "case \"$SHIFT_TASK\" in\n\
                     t1) echo 'compiler error in t1' >&2; exit 1 ;;\n\
                     t2) echo 'partial t2'; kill -9 $$ ;;\n\
                     esac\n\
                     echo \"summary of $SHIFT_TASK\"",
                ),
            ],
        )
        .committed();

    let output = repository.shift(&["build"]);

    assert_success(&output);
    assert_eq!(
        repository.log(),
        lines(&[
            "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "precheck build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "work build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "rollback build#1 task=t1 base=<start> outcome=work-failed cwd=<root> stdin=compiler error in t1|",
            "select build#2 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "precheck build#2 task=t2 base=<start> outcome=<unset> cwd=<root> stdin=t2|",
            "work build#2 task=t2 base=<start> outcome=<unset> cwd=<root> stdin=t2|",
            "rollback build#2 task=t2 base=<start> outcome=work-failed cwd=<root> stdin=partial t2|",
            "select build#3 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "precheck build#3 task=t3 base=<start> outcome=<unset> cwd=<root> stdin=t3|",
            "work build#3 task=t3 base=<start> outcome=<unset> cwd=<root> stdin=t3|",
            "verify build#3 task=t3 base=<start> outcome=<unset> cwd=<root> stdin=summary of t3|",
            "judge build#3 task=t3 base=<start> outcome=<unset> cwd=<root> stdin=summary of t3|",
            "commit build#3 task=t3 base=<start> outcome=verified cwd=<root> stdin=verdict on t3|",
            "select build#4 task=<unset> base=<verified t3> outcome=<unset> cwd=<root> stdin=",
        ])
    );
    assert_eq!(
        stdout(&output),
        lines(&[
            "iterations: 3",
            "commits: 1",
            "already-done: 0",
            "work-failed: 2",
            "verify-failed: 0",
            "judge-failed: 0",
            "declines: 0",
            "Exhausted",
        ])
    );
}

#[test]
fn test_declined_work_rolls_back_with_its_reason_and_ends_the_loop() {
    let select = select_tasks(&["t1", "t2"]);
    let repository = Repository::new()
        .with_loop(
            "build",
            &[
                ("select.sh", &select),
                (
                    "work.sh",
                    "echo 't1 contradicts the spec'\necho 'declining' >&2\nexit 3",
                ),
            ],
        )
        .committed();

    let output = repository.shift(&["build"]);

    assert_success(&output);
    assert_eq!(
        repository.log(),
        lines(&[
            "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "precheck build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "work build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "rollback build#1 task=t1 base=<start> outcome=declined cwd=<root> stdin=t1 contradicts the spec|",
        ])
    );
    assert_eq!(
        stdout(&output),
        lines(&[
            "iterations: 1",
            "commits: 0",
            "already-done: 0",
            "work-failed: 0",
            "verify-failed: 0",
            "judge-failed: 0",
            "declines: 1",
            "Declined t1",
        ])
    );
}

#[test]
fn test_failing_select_stops_the_run_with_its_stderr() {
    let repository = Repository::new()
        .with_loop(
            "build",
            &[("select.sh", "echo 't1'\necho 'selector broke' >&2\nexit 1")],
        )
        .committed();

    let output = repository.shift(&["build"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("Error: select failed (exit status: 1): selector broke\n"),
        "stderr must show the select error: {}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), "");
    assert_eq!(
        repository.log(),
        "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=\n"
    );
}

#[test]
fn test_failing_commit_stops_the_run() {
    let select = select_tasks(&["t1", "t2"]);
    let repository = Repository::new()
        .with_loop(
            "build",
            &[
                ("select.sh", &select),
                ("commit.sh", "echo 'index.lock exists' >&2\nexit 1"),
            ],
        )
        .committed();

    let output = repository.shift(&["build"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output)
            .contains("Error: commit failed for task t1 (exit status: 1): index.lock exists\n"),
        "stderr must show the commit error: {}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), "");
    assert_eq!(
        repository.log(),
        lines(&[
            "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "precheck build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "work build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "verify build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=summary of t1|",
            "judge build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=summary of t1|",
            "commit build#1 task=t1 base=<start> outcome=verified cwd=<root> stdin=verdict on t1|",
        ])
    );
}

#[test]
fn test_killed_rollback_stops_the_run() {
    let select = select_tasks(&["t1", "t2"]);
    let repository = Repository::new()
        .with_loop(
            "build",
            &[
                ("select.sh", &select),
                ("verify.sh", "echo 'tests failed' >&2\nexit 1"),
                ("rollback.sh", "kill -9 $$"),
            ],
        )
        .committed();

    let output = repository.shift(&["build"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("Error: rollback failed for task t1 (signal: 9 (SIGKILL)): \n"),
        "stderr must show the rollback error: {}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), "");
    assert_eq!(
        repository.log(),
        lines(&[
            "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "precheck build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "work build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "verify build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=summary of t1|",
            "rollback build#1 task=t1 base=<start> outcome=verify-failed cwd=<root> stdin=tests failed|",
        ])
    );
}

#[test]
fn test_max_iterations_ends_the_loop_with_exact_counts() {
    let select = select_tasks(&["t1", "t2", "t3"]);
    let repository = Repository::new()
        .with_loop(
            "build",
            &[
                ("select.sh", &select),
                (
                    "judge.sh",
                    "[ \"$SHIFT_TASK\" != t2 ] || exit 1\necho \"verdict on $SHIFT_TASK\"",
                ),
            ],
        )
        .committed();

    let output = repository.shift(&["build", "--max-iterations", "2"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        lines(&[
            "iterations: 2",
            "commits: 1",
            "already-done: 0",
            "work-failed: 0",
            "verify-failed: 0",
            "judge-failed: 1",
            "declines: 0",
            "MaxIterations",
        ])
    );
    assert_eq!(
        repository
            .log()
            .lines()
            .filter(|line| line.starts_with("select "))
            .collect::<Vec<_>>(),
        [
            "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "select build#2 task=<unset> base=<verified t1> outcome=<unset> cwd=<root> stdin=",
        ]
    );
}

#[test]
fn test_max_iterations_zero_runs_no_script() {
    let select = select_tasks(&["t1"]);
    let repository = Repository::new()
        .with_loop("build", &[("select.sh", &select)])
        .committed();

    let output = repository.shift(&["--max-iterations", "0", "build"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        lines(&[
            "iterations: 0",
            "commits: 0",
            "already-done: 0",
            "work-failed: 0",
            "verify-failed: 0",
            "judge-failed: 0",
            "declines: 0",
            "MaxIterations",
        ])
    );
    assert_eq!(repository.log(), "");
}

#[test]
fn test_plan_runs_only_the_plan_loop() {
    let select = select_tasks(&["topic-a"]);
    let repository = Repository::new()
        .with_loop("plan", &[("select.sh", &select)])
        .with_loop(
            "build",
            &[("select.sh", "echo 'the build loop ran' >&2\nexit 1")],
        )
        .committed();

    let output = repository.shift(&["plan"]);

    assert_success(&output);
    assert_eq!(
        repository.log(),
        lines(&[
            "select plan#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "precheck plan#1 task=topic-a base=<start> outcome=<unset> cwd=<root> stdin=topic-a|",
            "work plan#1 task=topic-a base=<start> outcome=<unset> cwd=<root> stdin=topic-a|",
            "verify plan#1 task=topic-a base=<start> outcome=<unset> cwd=<root> stdin=summary of topic-a|",
            "judge plan#1 task=topic-a base=<start> outcome=<unset> cwd=<root> stdin=summary of topic-a|",
            "commit plan#1 task=topic-a base=<start> outcome=verified cwd=<root> stdin=verdict on topic-a|",
            "select plan#2 task=<unset> base=<verified topic-a> outcome=<unset> cwd=<root> stdin=",
        ])
    );
    assert_eq!(
        stdout(&output).lines().last(),
        Some("Exhausted"),
        "the end state must be the last line"
    );
}

#[test]
fn test_missing_loop_directory_is_reported_by_its_path_before_any_script_runs() {
    let repository = Repository::new().with_loop("build", &[]).committed();

    let output = repository.shift(&["plan"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        format!(
            "Error: loop directory not found: {}\n",
            repository.root.join(".shifting/plan").display()
        )
    );
    assert_eq!(repository.log(), "");
}

#[test]
fn test_missing_script_is_reported_by_its_path_before_any_script_runs() {
    let repository = Repository::new()
        .with_loop_scripts(
            "build",
            &[],
            &[
                "select.sh",
                "precheck.sh",
                "work.sh",
                "verify.sh",
                "commit.sh",
                "rollback.sh",
            ],
        )
        .committed();

    let output = repository.shift(&["build"]);

    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        stderr(&output),
        format!(
            "Error: required script not found: {}\n",
            repository.root.join(".shifting/build/judge.sh").display()
        )
    );
    assert_eq!(repository.log(), "");
}

#[test]
fn test_repository_root_defaults_to_the_current_directory_resolved_through_git() {
    let repository = Repository::new().with_loop("build", &[]).committed();
    let subdirectory = repository.root.join("src");
    std::fs::create_dir(&subdirectory).expect("failed to create subdirectory");

    let output = Command::new(env!("CARGO_BIN_EXE_shift"))
        .arg("build")
        .current_dir(&subdirectory)
        .output()
        .expect("failed to run shift");

    assert_success(&output);
    assert_eq!(
        repository.log(),
        "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=\n"
    );
}

#[test]
fn test_task_and_outcome_from_shifts_environment_are_not_inherited() {
    let select = select_tasks(&["t1"]);
    let repository = Repository::new()
        .with_loop("build", &[("select.sh", &select)])
        .committed();

    let output = Command::new(env!("CARGO_BIN_EXE_shift"))
        .arg("--root")
        .arg(&repository.root)
        .arg("build")
        .env("SHIFT_TASK", "stale task")
        .env("SHIFT_OUTCOME", "stale outcome")
        .output()
        .expect("failed to run shift");

    assert_success(&output);
    assert_eq!(
        repository.log(),
        lines(&[
            "select build#1 task=<unset> base=<start> outcome=<unset> cwd=<root> stdin=",
            "precheck build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "work build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=t1|",
            "verify build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=summary of t1|",
            "judge build#1 task=t1 base=<start> outcome=<unset> cwd=<root> stdin=summary of t1|",
            "commit build#1 task=t1 base=<start> outcome=verified cwd=<root> stdin=verdict on t1|",
            "select build#2 task=<unset> base=<verified t1> outcome=<unset> cwd=<root> stdin=",
        ])
    );
}

#[test]
fn test_cli_parses_plan_and_build_with_default_options() {
    let parsed = ["plan", "build"]
        .map(|command| Cli::try_parse_from(["shift", command]).expect("failed to parse"));

    assert_eq!(
        parsed,
        [
            Cli {
                root: PathBuf::from("."),
                max_iterations: None,
                command: cli::Command::Plan,
            },
            Cli {
                root: PathBuf::from("."),
                max_iterations: None,
                command: cli::Command::Build,
            },
        ]
    );
}

#[test]
fn test_cli_global_options_parse_before_or_after_the_subcommand() {
    let parsed = [
        [
            "shift",
            "--root",
            "/srv/repo",
            "--max-iterations",
            "20",
            "build",
        ],
        [
            "shift",
            "build",
            "--root",
            "/srv/repo",
            "--max-iterations",
            "20",
        ],
    ]
    .map(|args| Cli::try_parse_from(args).expect("failed to parse"))
    .to_vec();

    assert_eq!(
        parsed,
        vec![
            Cli {
                root: PathBuf::from("/srv/repo"),
                max_iterations: Some(20),
                command: cli::Command::Build,
            };
            2
        ]
    );
}

#[test]
fn test_cli_rejects_the_prototype_options() {
    let errors = [
        vec!["shift", "--dir", ".shifting", "build"],
        vec!["shift", "build", "--verbose"],
    ]
    .map(|args| {
        Cli::try_parse_from(args)
            .expect_err("a prototype option must not parse")
            .kind()
    });

    assert_eq!(errors, [clap::error::ErrorKind::UnknownArgument; 2]);
}
