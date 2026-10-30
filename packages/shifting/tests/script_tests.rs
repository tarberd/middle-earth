use std::io::Write;
use std::path::{Path, PathBuf};

use shifting::branch::Branch;
use shifting::either::Either;
use shifting::step::{Step, StepExt};
use shifting::{Script, ScriptBranch, ScriptExit, ScriptOutput, ScriptStep};

/// Polls `condition` every 20ms for at most 10 seconds.
fn eventually(condition: impl Fn() -> bool) -> bool {
    (0..500).any(|_| {
        condition() || {
            std::thread::sleep(std::time::Duration::from_millis(20));
            false
        }
    })
}

fn create_executable_script(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    let path_str = path.to_str().expect("valid utf-8 path");
    let mut child = std::process::Command::new("sh")
        .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "_", path_str])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn script creator");
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content.as_bytes())
            .expect("failed to write script content");
    }
    let status = child.wait().expect("failed to wait on script creator");
    assert!(status.success(), "script creator failed with {status}");
    path
}

#[test]
fn test_script_step_exit_zero_transforms_stdin_to_stdout() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "echo_upper.sh",
        "#!/bin/sh\ntr '[:lower:]' '[:upper:]'\n",
    );

    let step = ScriptStep::new(&script_path);
    let output = step
        .apply("hello world".to_string())
        .expect("step execution failed");

    assert_eq!(output, "HELLO WORLD");
}

#[test]
fn test_script_output_trait_derivations() {
    let out1 = ScriptOutput::new(ScriptExit::Code(0), "ok", "warn");
    let out2 = ScriptOutput {
        exit: ScriptExit::Code(0),
        stdout: "ok".to_string(),
        stderr: "warn".to_string(),
    };

    assert_eq!(out1, out2);
    assert_eq!(out1.clone(), out2);
}

#[test]
fn test_script_step_non_zero_exit_returns_typed_error() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "fail.sh",
        "#!/bin/sh\necho 'compilation failed' >&2\nexit 42\n",
    );

    let step = ScriptStep::new(&script_path);
    let result = step.apply("source code".to_string());

    assert!(
        matches!(
            result,
            Err(shifting::ScriptError::Failed {
                exit: ScriptExit::Code(42),
                ref stderr,
            }) if stderr == "compilation failed\n"
        ),
        "expected a failure with exit code 42, got {result:?}"
    );
}

#[test]
fn test_script_step_killed_by_signal_fails_naming_the_signal() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path =
        create_executable_script(temp_dir.path(), "kill_self.sh", "#!/bin/sh\nkill -9 $$\n");

    let step = ScriptStep::new(&script_path);
    let error = step
        .apply("input".to_string())
        .expect_err("a kill by signal must fail the step");

    assert!(
        matches!(
            error,
            shifting::ScriptError::Failed {
                exit: ScriptExit::Signal(9),
                ..
            }
        ),
        "expected a failure by signal 9, got {error:?}"
    );
    assert_eq!(error.to_string(), "script failed (signal: 9 (SIGKILL)): ");
}

#[test]
fn test_script_step_missing_executable_returns_io_error() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing_path = temp_dir.path().join("non_existent_script.sh");

    let step = ScriptStep::new(&missing_path);
    let result = step.apply("input".to_string());

    assert!(
        matches!(result, Err(shifting::ScriptError::Io(ref err)) if err.kind() == std::io::ErrorKind::NotFound),
        "expected NotFound Io error, got {result:?}"
    );
}

#[test]
fn test_script_branch_exit_zero_returns_left() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "verify_ok.sh",
        "#!/bin/sh\nread -r payload\necho \"verified: $payload\"\nexit 0\n",
    );

    let branch = ScriptBranch::new(&script_path);
    let result = branch.evaluate("git diff".to_string());

    assert!(
        matches!(result, Ok(Either::Left(ref stdout)) if stdout == "verified: git diff\n"),
        "expected Either::Left(\"verified: git diff\\n\"), got {result:?}"
    );
}

#[test]
fn test_script_branch_non_zero_exit_returns_right_with_output() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "verify_fail.sh",
        "#!/bin/sh\necho 'test summary'\necho 'test failed details' >&2\nexit 1\n",
    );

    let branch = ScriptBranch::new(&script_path);
    let result = branch.evaluate("run-tests".to_string());

    assert_eq!(
        result.expect("a non-zero exit is not a bridge error"),
        Either::Right(ScriptOutput::new(
            ScriptExit::Code(1),
            "test summary\n",
            "test failed details\n"
        ))
    );
}

#[test]
fn test_script_branch_killed_by_signal_returns_right_naming_the_signal() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "kill_self_branch.sh",
        "#!/bin/sh\necho 'agent crashed' >&2\nkill -TERM $$\n",
    );

    let branch = ScriptBranch::new(&script_path);
    let result = branch.evaluate("input".to_string());

    assert_eq!(
        result.expect("a kill by signal is not a bridge error"),
        Either::Right(ScriptOutput::new(
            ScriptExit::Signal(15),
            "",
            "agent crashed\n"
        ))
    );
}

#[test]
fn test_script_branch_missing_executable_returns_io_error() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing_path = temp_dir.path().join("missing_branch_script.sh");

    let branch = ScriptBranch::new(&missing_path);
    let result = branch.evaluate("input".to_string());

    assert!(
        matches!(result, Err(shifting::ScriptError::Io(ref err)) if err.kind() == std::io::ErrorKind::NotFound),
        "expected NotFound Io error, got {result:?}"
    );
}

#[test]
fn test_script_step_and_branch_composition() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let work_script = create_executable_script(
        temp_dir.path(),
        "work.sh",
        "#!/bin/sh\nread -r task\necho \"done: $task\"\n",
    );
    let verify_pass_script = create_executable_script(
        temp_dir.path(),
        "verify_pass.sh",
        "#!/bin/sh\nread -r status\necho \"passed: $status\"\nexit 0\n",
    );
    let verify_fail_script = create_executable_script(
        temp_dir.path(),
        "verify_fail_comp.sh",
        "#!/bin/sh\necho 'tests failed' >&2\nexit 2\n",
    );

    let step = ScriptStep::new(&work_script);
    let branch_pass = ScriptBranch::new(&verify_pass_script);
    let branch_fail = ScriptBranch::new(&verify_fail_script);

    let pipeline_pass = step.clone().branch(branch_pass);
    let pass_res = pipeline_pass.evaluate("ticket-03".to_string());
    assert!(
        matches!(pass_res, Ok(Either::Left(ref stdout)) if stdout == "passed: done: ticket-03\n"),
        "expected Left(\"passed: done: ticket-03\\n\"), got {pass_res:?}"
    );

    let pipeline_fail = step.branch(branch_fail);
    let fail_res = pipeline_fail.evaluate("ticket-03".to_string());
    assert!(
        matches!(fail_res, Ok(Either::Right(ref out)) if out.exit == ScriptExit::Code(2) && out.stderr == "tests failed\n"),
        "expected Right with code 2 and stderr, got {fail_res:?}"
    );
}

#[test]
fn test_script_step_invalid_utf8_stdout_fails_the_script() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "invalid_utf8.sh",
        "#!/bin/sh\nprintf '\\xff\\xff'\n",
    );

    let step = ScriptStep::new(&script_path);
    let result = step.apply("input".to_string());

    assert!(
        matches!(
            result,
            Err(shifting::ScriptError::Failed {
                exit: ScriptExit::InvalidStdout,
                ..
            })
        ),
        "expected a failure for invalid stdout, got {result:?}"
    );
}

#[test]
fn test_script_branch_invalid_utf8_stdout_returns_right_with_lossy_stdout() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "invalid_utf8_branch.sh",
        "#!/bin/sh\nprintf 'ok \\xff'\n",
    );

    let branch = ScriptBranch::new(&script_path);
    let result = branch.evaluate("input".to_string());

    assert_eq!(
        result.expect("invalid stdout is not a bridge error"),
        Either::Right(ScriptOutput::new(
            ScriptExit::InvalidStdout,
            "ok \u{fffd}",
            ""
        ))
    );
}

#[test]
fn test_script_branch_killed_by_signal_with_invalid_stdout_names_the_signal() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "invalid_utf8_killed.sh",
        "#!/bin/sh\nprintf '\\xff'\nkill -9 $$\n",
    );

    let branch = ScriptBranch::new(&script_path);
    let result = branch.evaluate("input".to_string());

    assert!(
        matches!(result, Ok(Either::Right(ref out)) if out.exit == ScriptExit::Signal(9)),
        "expected Right naming signal 9, got {result:?}"
    );
}

#[test]
fn test_script_step_large_payload_streaming_no_deadlock() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path =
        create_executable_script(temp_dir.path(), "stream_echo.sh", "#!/bin/sh\ncat\n");

    let step = ScriptStep::new(&script_path);
    // 128KB payload exceeds default 64KB OS pipe buffer
    let payload = "x".repeat(128 * 1024);
    let output = step.apply(payload.clone()).expect("step execution failed");

    assert_eq!(output.len(), 128 * 1024);
    assert_eq!(output, payload);
}

#[test]
fn test_script_branch_large_payload_streaming_no_deadlock() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path =
        create_executable_script(temp_dir.path(), "stream_branch.sh", "#!/bin/sh\ncat\n");

    let branch = ScriptBranch::new(&script_path);
    let payload = "y".repeat(128 * 1024);
    let result = branch
        .evaluate(payload.clone())
        .expect("branch execution failed");

    assert_eq!(result, Either::Left(payload));
}

#[test]
fn test_script_error_display_and_source() {
    use std::error::Error;

    let io_err = shifting::ScriptError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "not found",
    ));
    assert_eq!(format!("{io_err}"), "I/O error: not found");
    assert!(io_err.source().is_some());

    let failed = shifting::ScriptError::Failed {
        exit: ScriptExit::Code(1),
        stderr: "fail".to_string(),
    };
    assert_eq!(format!("{failed}"), "script failed (exit status: 1): fail");
    assert!(failed.source().is_none());
}

#[test]
fn test_script_step_broken_pipe_early_exit_completes_ok() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path =
        create_executable_script(temp_dir.path(), "early_exit.sh", "#!/bin/sh\nexit 0\n");

    let step = ScriptStep::new(&script_path);
    // 512KB payload ensures pipe buffer is exceeded and BrokenPipe is encountered
    let payload = "x".repeat(512 * 1024);
    let output = step
        .apply(payload)
        .expect("step execution failed on broken pipe");

    assert_eq!(output, "");
}

#[test]
fn test_script_branch_broken_pipe_early_exit_completes_ok() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "head_exit.sh",
        "#!/bin/sh\nhead -c 1 > /dev/null\necho 'early branch ok'\nexit 0\n",
    );

    let branch = ScriptBranch::new(&script_path);
    // 512KB payload ensures pipe buffer is exceeded and BrokenPipe is encountered
    let payload = "y".repeat(512 * 1024);
    let result = branch
        .evaluate(payload)
        .expect("branch execution failed on broken pipe");

    assert_eq!(result, Either::Left("early branch ok\n".to_string()));
}

#[test]
fn test_script_forwards_stderr_to_sink_before_exiting() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let release = temp_dir.path().join("release");
    let sink_path = temp_dir.path().join("sink.log");
    let release_str = release.to_str().expect("valid utf-8 path");
    let script_path = create_executable_script(
        temp_dir.path(),
        "long_running.sh",
        &format!(
            "#!/bin/sh\n\
             echo 'agent started' >&2\n\
             i=0\n\
             while [ ! -e \"{release_str}\" ] && [ \"$i\" -lt 400 ]; do sleep 0.05; i=$((i + 1)); done\n\
             echo 'agent finished' >&2\n\
             echo 'summary'\n"
        ),
    );

    let mut sink = std::fs::File::create(&sink_path).expect("failed to create sink");
    let script = Script::new(&script_path);
    let running = std::thread::spawn(move || script.run("", &mut sink));

    let forwarded_while_running = eventually(|| {
        std::fs::read_to_string(&sink_path).is_ok_and(|forwarded| forwarded == "agent started\n")
    });
    std::fs::File::create(&release).expect("failed to release script");
    let output = running
        .join()
        .expect("script thread panicked")
        .expect("script run failed");

    assert!(
        forwarded_while_running,
        "stderr must reach the sink before the script exits"
    );
    assert_eq!(output.stdout, "summary\n");
    assert_eq!(output.stderr, "agent started\nagent finished\n");
    assert_eq!(
        std::fs::read_to_string(&sink_path).expect("failed to read sink"),
        "agent started\nagent finished\n"
    );
}

#[test]
fn test_script_receives_invocation_environment_on_top_of_inherited() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "print_env.sh",
        "#!/bin/sh\nprintf '%s|%s|%s' \"$SHIFT_TASK\" \"$SHIFT_LOOP\" \"$PATH\"\n",
    );

    let output = Script::new(&script_path)
        .with_env("SHIFT_TASK", "task-07")
        .with_env("SHIFT_LOOP", "build")
        .run("", &mut std::io::sink())
        .expect("script run failed");

    let inherited_path = std::env::var("PATH").expect("PATH must be set for tests");
    assert_eq!(output.stdout, format!("task-07|build|{inherited_path}"));
}

#[test]
fn test_script_runs_in_invocation_working_directory() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let repository = temp_dir.path().join("repository");
    std::fs::create_dir(&repository).expect("failed to create working directory");
    std::fs::write(repository.join("task.md"), "relative read\n").expect("failed to write file");
    let script_path = create_executable_script(
        temp_dir.path(),
        "read_relative.sh",
        "#!/bin/sh\ncat task.md\n",
    );

    let output = Script::new(&script_path)
        .with_current_dir(&repository)
        .run("", &mut std::io::sink())
        .expect("script run failed");

    assert_eq!(output.stdout, "relative read\n");
}

#[test]
fn test_script_invalid_utf8_stderr_is_decoded_lossily() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "invalid_stderr.sh",
        "#!/bin/sh\nprintf 'bad \\377 byte\\n' >&2\necho 'ok'\n",
    );

    let mut forwarded = Vec::new();
    let output = Script::new(&script_path)
        .run("", &mut forwarded)
        .expect("invalid stderr is not a bridge error");

    assert_eq!(
        output,
        ScriptOutput::new(ScriptExit::Code(0), "ok\n", "bad \u{fffd} byte\n")
    );
    assert_eq!(forwarded, b"bad \xff byte\n");
}

#[test]
fn test_script_large_stdout_and_stderr_at_once_complete_in_full() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    // 1MiB on each stream exceeds the 64KB OS pipe buffer many times over.
    let script_path = create_executable_script(
        temp_dir.path(),
        "flood.sh",
        "#!/bin/sh\n\
         head -c 1048576 /dev/zero | tr '\\0' 'o' &\n\
         head -c 1048576 /dev/zero | tr '\\0' 'e' >&2\n\
         wait\n",
    );

    let script = Script::new(&script_path);
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut forwarded = Vec::new();
        let output = script.run("", &mut forwarded);
        sender.send((output, forwarded))
    });
    let (output, forwarded) = receiver
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("script run deadlocked");

    assert_eq!(
        output.expect("script run failed"),
        ScriptOutput::new(
            ScriptExit::Code(0),
            "o".repeat(1024 * 1024),
            "e".repeat(1024 * 1024)
        )
    );
    assert_eq!(forwarded, "e".repeat(1024 * 1024).into_bytes());
}

#[test]
fn test_failure_report_is_the_stderr_of_the_failing_script() {
    let output = ScriptOutput::new(
        ScriptExit::Code(1),
        "test summary\n",
        "test failed details\n",
    );

    assert_eq!(output.failure_report(), "test failed details\n");
}

#[test]
fn test_failure_report_falls_back_to_stdout_when_stderr_is_empty() {
    let output = ScriptOutput::new(ScriptExit::Signal(9), "partial summary\n", "");

    assert_eq!(output.failure_report(), "partial summary\n");
}
